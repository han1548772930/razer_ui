//! Current 70 XT / 226 es -> 4230: integer stepping and local row previews.
use super::*;
use gpui_kit::base::{Button as BaseButton, NumberInput, StepAction};
use gpui_kit::prelude::FluentBuilder as _;
use std::time::Duration;

#[derive(Default)]
pub(super) struct EditState {
    pub(super) registered: bool,
    pub(super) typed: bool,
    repeat: Option<Task<()>>,
    suppress_click: bool,
}

impl MouseProductWorkspace {
    pub(super) fn finish_dpi_number(&mut self, path: &str) {
        if let Some(state) = self.dpi_numbers.get_mut(path) {
            *state = EditState::default();
        }
    }

    fn stop_dpi_number_repeat(&mut self, path: &str) {
        if let Some(state) = self.dpi_numbers.get_mut(path) {
            state.repeat = None;
        }
    }

    fn step_dpi_number(
        &mut self,
        path: &str,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.dpi_number_editable(path) {
            return false;
        }
        let Some(input) = self.inputs.get(path).cloned() else {
            return false;
        };
        let Some(state) = self.dpi_numbers.get(path) else {
            return false;
        };
        let text = input.read(cx).value().to_string();
        let current = text.parse::<f32>().unwrap_or(0.);
        let preview = state.registered;
        // 4230 stores handleChange text as a string. Its integer volumeUp uses
        // JS + before parseInput; preserve concatenation for the first typed step.
        let next = match action {
            StepAction::Increment if state.typed => format!("{text}{}", self.spec.dpi_step)
                .parse::<f32>()
                .unwrap_or(0.),
            StepAction::Increment => current + self.spec.dpi_step as f32,
            StepAction::Decrement => current - self.spec.dpi_step as f32,
        };
        let step = self.spec.dpi_step as f32;
        let value =
            ((next / step).ceil() * step).clamp(self.spec.min_dpi as f32, self.spec.max_dpi as f32);
        let prop_changed = self
            .sliders
            .get(path)
            .is_some_and(|slider| slider.read(cx).value().start() != value);
        // 4230.componentDidUpdate leaves typed text intact when the row value
        // did not change. handleBlur's isKeyInput path canonicalizes it later.
        let refresh_text = !state.typed || prop_changed;
        if refresh_text {
            self.dpi_numbers.get_mut(path).unwrap().typed = false;
        }
        if !preview {
            self.write_number(path, value, window, cx);
        }
        // 70 EI / 226 ls keep focused stepping local until 4230.handleBlur.
        // Do not select a stage or persist a profile while registered is true.
        self.syncing = true;
        if refresh_text {
            input.update(cx, |input, cx| {
                input.set_value((value as i64).to_string(), window, cx)
            });
        }
        if let Some(slider) = self.sliders.get(path) {
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        self.syncing = false;
        cx.notify();
        !prop_changed || (value != self.spec.min_dpi as f32 && value != self.spec.max_dpi as f32)
    }

    fn start_dpi_number_repeat(
        &mut self,
        path: String,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_dpi_number_repeat(&path);
        if !self.step_dpi_number(&path, action, window, cx) {
            return;
        }
        let repeat_path = path.clone();
        let task = cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let more = view
                    .update_in(cx, |view, window, cx| {
                        window.is_window_active()
                            && view.step_dpi_number(&repeat_path, action, window, cx)
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        });
        self.dpi_numbers.get_mut(&path).unwrap().repeat = Some(task);
    }
}

#[derive(IntoElement)]
pub(super) struct DpiNumber {
    pub(super) path: String,
    pub(super) group: SharedString,
    pub(super) input: Entity<InputState>,
    pub(super) owner: WeakEntity<MouseProductWorkspace>,
    pub(super) disabled: bool,
    pub(super) min: u32,
    pub(super) max: u32,
    pub(super) typed: bool,
}

impl RenderOnce for DpiNumber {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self.input.focus_handle(cx);
        let focused = focus.is_focused(window);
        let disabled = self.disabled;
        let value = self.input.read(cx).value().parse::<u32>().unwrap_or(0);
        let arrow_owner = self.owner.clone();
        let arrow_path = self.path.clone();
        let group = self.group.clone();
        let arrow = move |button: BaseButton, asset, top, action, limit| {
            let down = arrow_owner.clone();
            let up = arrow_owner.clone();
            let out = arrow_owner.clone();
            let leave = arrow_owner.clone();
            let click = arrow_owner.clone();
            let down_path = arrow_path.clone();
            let up_path = arrow_path.clone();
            let out_path = arrow_path.clone();
            let leave_path = arrow_path.clone();
            let click_path = arrow_path.clone();
            let button = button
                .w(surface::css(14.))
                .h(surface::css(12.))
                .p_0()
                .relative()
                .disabled(disabled || limit)
                .when(limit, |button| button.opacity(0.3))
                .when(!focused, |button| {
                    button
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                })
                .hover(|s| s.bg(rgba(0xffffff1a)))
                .active(|s| s.bg(rgba(0x0000001a)))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if disabled || limit {
                        return;
                    }
                    let _ = down.update(cx, |owner, cx| {
                        owner.start_dpi_number_repeat(down_path.clone(), action, window, cx)
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let _ = up.update(cx, |owner, _| owner.stop_dpi_number_repeat(&up_path));
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    let _ = out.update(cx, |owner, _| owner.stop_dpi_number_repeat(&out_path));
                })
                .on_hover(move |hovered, _, cx| {
                    if !hovered {
                        let _ =
                            leave.update(cx, |owner, _| owner.stop_dpi_number_repeat(&leave_path));
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
                    let _ = click.update(cx, |owner, _| {
                        if let Some(state) = owner.dpi_numbers.get_mut(&click_path) {
                            state.suppress_click = true;
                        }
                    });
                }
            })
        };
        let increment = arrow.clone();
        let click_owner = self.owner.clone();
        let click_path = self.path.clone();
        let click_input = self.input.clone();
        let wheel_owner = self.owner.clone();
        let wheel_path = self.path.clone();
        let owner = self.owner;
        let path = self.path;
        div()
            .id(SharedString::from(format!("dpi-number-{path}")))
            .track_focus(&focus)
            .relative()
            .w(surface::css(62.))
            .h(surface::css(26.))
            .ml(surface::css(10.))
            .flex_shrink_0()
            .border_1()
            .bg(rgb(0x111111))
            .text_color(rgb(0xcccccc))
            .border_color(if focused { rgb(0x44d62c) } else { rgba(0) })
            .group_hover(self.group, |s| s.border_color(rgb(0xcccccc)))
            .hover(|s| s.border_color(rgb(0x44d62c)))
            .on_key_down(move |event, window, cx| {
                if !disabled && matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                    window.blur(cx);
                    cx.stop_propagation();
                }
            })
            .on_scroll_wheel(move |event, window, cx| {
                if disabled || !focus.is_focused(window) {
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
                let handled = wheel_owner.update(cx, |owner, cx| {
                    if !owner
                        .dpi_numbers
                        .get(&wheel_path)
                        .is_some_and(|state| state.registered)
                    {
                        // Tab focus alone never registers the source wheel listener.
                        return false;
                    }
                    // 4230 wheel guards props.value (the row preview), not
                    // newly typed text; keyboard/button stepping is separate.
                    let Some(slider) = owner.sliders.get(&wheel_path) else {
                        return false;
                    };
                    let value = slider.read(cx).value().start();
                    if (action == StepAction::Increment && value >= owner.spec.max_dpi as f32)
                        || (action == StepAction::Decrement && value <= owner.spec.min_dpi as f32)
                    {
                        return true;
                    }
                    owner.step_dpi_number(&wheel_path, action, window, cx);
                    true
                });
                if handled.unwrap_or(false) {
                    cx.stop_propagation();
                }
            })
            .child(
                NumberInput::new(&self.input)
                    .disabled(disabled)
                    .size_full()
                    .controls_right()
                    .input(
                        div()
                            .id("text")
                            .on_click(move |_, window, cx| {
                                if disabled {
                                    return;
                                }
                                let _ = click_owner.update(cx, |owner, cx| {
                                    if let Some(state) = owner.dpi_numbers.get_mut(&click_path) {
                                        if !state.registered {
                                            state.registered = true;
                                            click_input.update(cx, |input, cx| {
                                                input.select_all(window, cx)
                                            });
                                        }
                                    }
                                });
                            })
                            .child(
                                Input::new(&self.input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .disabled(disabled)
                                    .h(surface::css(24.))
                                    .w_full()
                                    .p_0()
                                    .pl(surface::css(5.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(14.)),
                            ),
                    )
                    .increment_button(move |button| {
                        increment(
                            button,
                            "synapse/stepper-up.svg",
                            5.,
                            StepAction::Increment,
                            !self.typed && value == self.max,
                        )
                    })
                    .decrement_button(move |button| {
                        arrow(
                            button,
                            "synapse/stepper-down.svg",
                            3.,
                            StepAction::Decrement,
                            !self.typed && value == self.min,
                        )
                    })
                    .on_step(move |action, window, cx| {
                        let _ = owner.update(cx, |owner, cx| {
                            let Some(state) = owner.dpi_numbers.get_mut(&path) else {
                                return;
                            };
                            if std::mem::take(&mut state.suppress_click) {
                                return;
                            }
                            owner.step_dpi_number(&path, action, window, cx);
                        });
                    }),
            )
    }
}
