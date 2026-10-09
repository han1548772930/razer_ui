//! 182 IM -> jm -> Vm: five retained DPI slots, per-slot XY and visibility.
//! Native GPUI drag/drop owns capture; slot IDs preserve controls across reorder.
use super::{settings::Sensitivity, workspace::DeviceWorkspace};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::{NumberInput, StepAction};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    input::{Input, InputEvent, InputState},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;
use razer_widgets::surface::SynapseSwitch;
use std::{collections::BTreeMap, time::Duration};

// 182 Vm passes maxLength=6 to 4230. The optional minus is not counted.
fn valid_dpi_draft(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    digits.len() <= 6 && digits.bytes().all(|byte| byte.is_ascii_digit())
}

// 4230 parseInput: empty/invalid => 0, ceil to dpiStep, then clamp.
fn parse_dpi_draft(text: &str) -> u32 {
    normalize_dpi_draft(text.parse::<i64>().unwrap_or(0))
}

fn normalize_dpi_draft(value: i64) -> u32 {
    let value = value.clamp(100, 30000) as u32;
    value.div_ceil(50) * 50
}

#[cfg(test)]
mod draft_tests {
    use super::{parse_dpi_draft, valid_dpi_draft};

    #[test]
    fn source_dpi_drafts_preserve_partial_input_and_round_only_on_commit() {
        for draft in ["", "-", "0", "101", "999999", "-999999"] {
            assert!(valid_dpi_draft(draft), "{draft}");
        }
        for draft in ["1000000", "-1000000", "1.5", "+1", "1e3", "１２"] {
            assert!(!valid_dpi_draft(draft), "{draft}");
        }
        for (draft, expected) in [
            ("", 100),
            ("-", 100),
            ("-150", 100),
            ("101", 150),
            ("00150", 150),
            ("29951", 30000),
            ("999999", 30000),
        ] {
            assert_eq!(parse_dpi_draft(draft), expected, "{draft}");
        }
    }
}

struct SlotControls {
    inputs: [Entity<InputState>; 2],
    sliders: [Entity<SliderState>; 2],
    focus: FocusHandle,
    hovered: bool,
    focused: bool,
    repeat_tasks: [Option<Task<()>>; 2],
    repeat_actions: [Option<StepAction>; 2],
    suppress_pointer_click: [bool; 2],
}

pub(super) struct SensitivityControls {
    slots: BTreeMap<u8, SlotControls>,
    _subscriptions: Vec<Subscription>,
}

impl SensitivityControls {
    pub(super) fn new(window: &mut Window, cx: &mut Context<DeviceWorkspace>) -> Self {
        let mut slots = BTreeMap::new();
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.observe_window_activation(window, |owner, window, _| {
            if !window.is_window_active() {
                for controls in owner.sensitivity_controls.slots.values_mut() {
                    controls.repeat_actions = [None, None];
                    controls.repeat_tasks = [None, None];
                }
            }
        }));
        for id in 1..=5 {
            let focus = cx.focus_handle().tab_stop(true);
            subscriptions.push(cx.on_focus_in(&focus, window, move |owner, _, cx| {
                owner
                    .sensitivity_controls
                    .slots
                    .get_mut(&id)
                    .unwrap()
                    .focused = true;
                cx.notify();
            }));
            subscriptions.push(cx.on_focus_out(&focus, window, move |owner, _, _, cx| {
                owner
                    .sensitivity_controls
                    .slots
                    .get_mut(&id)
                    .unwrap()
                    .focused = false;
                cx.notify();
            }));
            let inputs = std::array::from_fn(|axis| {
                let input = cx.new(|cx| {
                    InputState::new(window, cx).validate(|text, _| valid_dpi_draft(text))
                });
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |owner, _, event, window, cx| {
                        if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                            owner.stop_dpi_repeat(id, axis);
                            owner.commit_dpi_draft(id, axis, window, cx);
                            if matches!(event, InputEvent::PressEnter { .. }) {
                                window.blur(cx);
                            }
                        }
                    },
                ));
                input
            });
            let sliders = std::array::from_fn(|axis| {
                let slider = cx.new(|_| SliderState::new().min(100.).max(30000.).step(50.));
                subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |owner, _, event: &SliderEvent, window, cx| {
                        let SliderEvent::Change(value) = event else {
                            return;
                        };
                        let value = value.start().round() as u32;
                        let state = &owner.settings().sensitivity;
                        let Some(index) = state.editable_slot(id) else {
                            owner.sync_controls(window, cx);
                            return;
                        };
                        if state.stages[index][axis] != value {
                            owner
                                .edit(window, cx, |s| s.sensitivity.set_slot_axis(id, axis, value));
                        }
                    },
                ));
                slider
            });
            slots.insert(
                id,
                SlotControls {
                    inputs,
                    sliders,
                    focus,
                    hovered: false,
                    focused: false,
                    repeat_tasks: [None, None],
                    repeat_actions: [None, None],
                    suppress_pointer_click: [false, false],
                },
            );
        }
        Self {
            slots,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn sync(&self, state: &Sensitivity, window: &mut Window, cx: &mut App) {
        for (slot, values) in state.slots.iter().zip(&state.stages) {
            let controls = &self.slots[&slot.id];
            for (axis, value) in values.iter().enumerate() {
                let text = value.to_string();
                if controls.inputs[axis].read(cx).value().as_str() != text {
                    controls.inputs[axis].update(cx, |input, cx| input.set_value(text, window, cx));
                }
                if controls.sliders[axis].read(cx).value().start() != *value as f32 {
                    controls.sliders[axis]
                        .update(cx, |slider, cx| slider.set_value(*value as f32, window, cx));
                }
            }
        }
    }
}

/// 182 `.stage .stepper`: Base keeps the retained editor, spinbutton actions,
/// focus and two button slots; this surface supplies the source's presentation.
#[derive(IntoElement)]
struct DpiNumberInput {
    id: u8,
    axis: usize,
    label: SharedString,
    disabled: bool,
    show_controls: bool,
    input: Entity<InputState>,
    owner: WeakEntity<DeviceWorkspace>,
}

impl RenderOnce for DpiNumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let axis = self.axis;
        let disabled = self.disabled;
        let focus = self.input.focus_handle(cx);
        let focused = focus.is_focused(window) && !disabled;
        let show_controls = self.show_controls || focused;
        let value = self.input.read(cx).value().parse::<i64>().unwrap_or(0);
        let hover = razer_widgets::theme::DropdownColors::new().hover();
        let active = cx.theme().button_primary_foreground.opacity(0.1);
        let arrow_owner = self.owner.clone();
        let arrow = move |button: BaseButton, asset, at_limit, top, action| {
            let opacity = if at_limit { 0.3 } else { 1. };
            let down = arrow_owner.clone();
            let up = arrow_owner.clone();
            let outside = arrow_owner.clone();
            let leave = arrow_owner.clone();
            let click = arrow_owner.clone();
            let button = button
                .w(surface::css(14.))
                .h(surface::css(12.))
                .p_0()
                .relative()
                .opacity(opacity)
                .when(!show_controls, |button| {
                    button
                        .invisible()
                        .group_hover("dpi-stepper", |style| style.visible())
                })
                .when(!disabled && !at_limit, |button| {
                    button
                        .hover(move |style| style.bg(hover))
                        .active(move |style| style.bg(active))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            let _ = down.update(cx, |owner, cx| {
                                owner.start_dpi_repeat(id, axis, action, window, cx)
                            });
                        })
                })
                // Keep cancellation listeners when a press reaches a bound or
                // the slot becomes disabled before the pointer is released.
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let _ = up.update(cx, |owner, _| owner.stop_dpi_repeat(id, axis));
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    let _ = outside.update(cx, |owner, _| owner.stop_dpi_repeat(id, axis));
                })
                .on_hover(move |hovered, _, cx| {
                    if !*hovered {
                        let _ = leave.update(cx, |owner, _| owner.stop_dpi_repeat(id, axis));
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
            // Base appends its semantic click handler after decorated pointer
            // listeners. The press already stepped; consume only the subsequent
            // mouse click, leaving keyboard/touch/accessibility stepping intact.
            StatefulInteractiveElement::on_click(button, move |event, _, cx| {
                if !disabled && !at_limit && matches!(event, ClickEvent::Mouse(_)) {
                    let _ = click.update(cx, |owner, _| {
                        owner
                            .sensitivity_controls
                            .slots
                            .get_mut(&id)
                            .unwrap()
                            .suppress_pointer_click[axis] = true;
                    });
                }
            })
        };
        let increment_arrow = arrow.clone();
        let owner = self.owner.clone();
        let wheel_owner = self.owner;
        let click_input = self.input.clone();
        div()
            .id(SharedString::from(format!("dpi-slot-{id}-stepper-{axis}")))
            .test_support()
            .group("dpi-stepper")
            .track_focus(&focus)
            .w(surface::css(62.))
            .h(surface::css(26.))
            .flex_shrink_0()
            .border_1()
            .border_color(if focused {
                cx.theme().primary
            } else if self.show_controls {
                cx.theme().foreground
            } else {
                cx.theme().transparent
            })
            .bg(cx.theme().group_box)
            .when(!disabled, |element| {
                element.hover(|style| style.border_color(cx.theme().primary))
            })
            .on_key_down(move |event, window, cx| {
                if !disabled && event.keystroke.key == "escape" {
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
                let _ = wheel_owner.update(cx, |owner, cx| {
                    owner.step_dpi_value(id, axis, action, window, cx)
                });
                cx.stop_propagation();
            })
            .child(
                NumberInput::new(&self.input)
                    .disabled(disabled)
                    .size_full()
                    .controls_right()
                    .input(
                        div()
                            .id("dpi-text")
                            .on_click(move |_, window, cx| {
                                if !disabled && !focused {
                                    click_input.update(cx, |input, cx| {
                                        input.select_all(window, cx);
                                    });
                                }
                            })
                            .child(
                                Input::new(&self.input)
                                    .id(SharedString::from(format!("dpi-slot-{id}-input-{axis}")))
                                    .aria_label(self.label.clone())
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
                        increment_arrow(
                            button.accessibility_label("增加 DPI"),
                            "synapse/stepper-up.svg",
                            value >= 30000,
                            5.,
                            StepAction::Increment,
                        )
                    })
                    .decrement_button(move |button| {
                        arrow(
                            button.accessibility_label("减少 DPI"),
                            "synapse/stepper-down.svg",
                            value <= 100,
                            3.,
                            StepAction::Decrement,
                        )
                    })
                    .on_step(move |action, window, cx| {
                        let _ = owner.update(cx, |owner, cx| {
                            let controls = owner.sensitivity_controls.slots.get_mut(&id).unwrap();
                            if std::mem::take(&mut controls.suppress_pointer_click[axis]) {
                                return;
                            }
                            owner.step_dpi_value(id, axis, action, window, cx)
                        });
                    }),
            )
    }
}

#[derive(Clone)]
struct DpiDrag {
    device: String,
    profile: String,
    slot: u8,
    position: usize,
    label: String,
}
impl Render for DpiDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().primary)
            .child(self.label.clone())
    }
}

impl DeviceWorkspace {
    fn stop_dpi_repeat(&mut self, id: u8, axis: usize) {
        let controls = self.sensitivity_controls.slots.get_mut(&id).unwrap();
        controls.repeat_actions[axis] = None;
        controls.repeat_tasks[axis].take();
    }

    fn start_dpi_repeat(
        &mut self,
        id: u8,
        axis: usize,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_dpi_repeat(id, axis);
        let state = &self.settings().sensitivity;
        let Some(index) = state.editable_slot(id) else {
            return;
        };
        if axis == 1 && !state.slots[index].independent {
            return;
        }
        let device = self.identity();
        let profile = self.device().active_profile.clone();
        // Module 4230 applies once on mouse-down and repeats every 300 ms.
        self.step_dpi_value(id, axis, action, window, cx);
        self.sensitivity_controls
            .slots
            .get_mut(&id)
            .unwrap()
            .repeat_actions[axis] = Some(action);
        let task = cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let keep_going = view
                    .update_in(cx, |owner, window, cx| {
                        if owner.identity() != device
                            || owner.device().active_profile != profile
                            || owner.page != crate::nav::Tab::Performance
                            || !window.is_window_active()
                            || !owner.sensitivity_controls.slots[&id].inputs[axis]
                                .focus_handle(cx)
                                .is_focused(window)
                            || owner.sensitivity_controls.slots[&id].repeat_actions[axis]
                                != Some(action)
                        {
                            return false;
                        }
                        let state = &owner.settings().sensitivity;
                        let Some(index) = state.editable_slot(id) else {
                            return false;
                        };
                        if axis == 1 && !state.slots[index].independent {
                            return false;
                        }
                        let value = state.stages[index][axis];
                        if (action == StepAction::Increment && value == 30000)
                            || (action == StepAction::Decrement && value == 100)
                        {
                            return false;
                        }
                        owner.step_dpi_value(id, axis, action, window, cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        });
        self.sensitivity_controls
            .slots
            .get_mut(&id)
            .unwrap()
            .repeat_tasks[axis] = Some(task);
    }

    fn commit_dpi_draft(
        &mut self,
        id: u8,
        axis: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = &self.sensitivity_controls.slots[&id].inputs[axis];
        let value = parse_dpi_draft(&input.read(cx).value());
        self.set_dpi_value(id, axis, value, window, cx);
    }

    fn set_dpi_value(
        &mut self,
        id: u8,
        axis: usize,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &self.settings().sensitivity;
        let Some(index) = state.editable_slot(id) else {
            self.sync_controls(window, cx);
            return;
        };
        // Hidden Y controls and delayed events must not change a linked slot.
        if axis == 1 && !state.slots[index].independent {
            self.sync_controls(window, cx);
            return;
        }
        if state.stages[index][axis] != value {
            self.edit(window, cx, |settings| {
                settings.sensitivity.set_slot_axis(id, axis, value)
            });
        } else {
            // Canonicalize drafts such as "00100" even when the domain is unchanged.
            self.sync_controls(window, cx);
        }
    }

    fn step_dpi_value(
        &mut self,
        id: u8,
        axis: usize,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &self.settings().sensitivity;
        let Some(index) = state.editable_slot(id) else {
            return;
        };
        if axis == 1 && !state.slots[index].independent {
            return;
        }
        let input = self.sensitivity_controls.slots[&id].inputs[axis].clone();
        let current = input.read(cx).value().parse::<i64>().unwrap_or(0);
        let value = normalize_dpi_draft(match action {
            StepAction::Increment => current.saturating_add(50),
            StepAction::Decrement => current.saturating_sub(50),
        });
        self.set_dpi_value(id, axis, value, window, cx);
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub(super) fn sensitivity_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.settings().sensitivity;
        surface::panel(razer_i18n::t("SENSITIVITY_HEADER"), cx)
            .id("sensitivity-panel")
            .test_support()
            .gap_0()
            .child(
                div()
                    .mt(surface::css(16.))
                    .child(razer_i18n::t("SENSITIVITY_DESC")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(surface::css(10.))
                    .h(surface::css(27.))
                    .mt(surface::css(25.))
                    .child(razer_i18n::t("SENSITIVITY_STAGES"))
                    .child(
                        SynapseSwitch::new("dpi-stages-visible")
                            .accessibility_label(razer_i18n::t("SENSITIVITY_STAGES"))
                            .checked(state.visible)
                            .on_change(cx.listener(|owner, next, w, cx| {
                                owner.edit(w, cx, |s| s.sensitivity.visible = *next);
                            })),
                    ),
            )
            .child(
                div()
                    .id("sensitivity-header")
                    .test_support()
                    .relative()
                    .h(surface::css(20.))
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(50.))
                            .bottom_0()
                            .child("DPI"),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(110.))
                            .bottom_0()
                            .child("100"),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(375.))
                            .bottom_0()
                            .child("30000"),
                    ),
            )
            .child(
                div().flex().flex_col().children(
                    state
                        .slots
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| state.visible || *index == state.active)
                        .map(|(index, _)| self.sensitivity_slot(index, cx)),
                ),
            )
            .into_any_element()
    }

    fn sensitivity_slot(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.settings().sensitivity;
        let slot = &state.slots[index];
        let id = slot.id;
        let selected = index == state.active;
        let ordinal = if state.visible {
            state.slots[..=index].iter().filter(|s| s.enabled).count()
        } else {
            1
        };
        let independent = slot.independent;
        let enabled = slot.enabled;
        let controls = &self.sensitivity_controls.slots[&id];
        let show_actions = controls.hovered || controls.focused;
        let label = format!(
            "{} {}",
            razer_i18n::t("STAGE"),
            if enabled { ordinal } else { index + 1 }
        );
        let drag = DpiDrag {
            device: self.identity(),
            profile: self.device().active_profile.clone(),
            slot: id,
            position: index,
            label: label.clone(),
        };
        let stage_button = BaseButton::new(SharedString::from(format!("dpi-stage-{id}")))
            .accessibility_label(label.clone())
            .selected(selected)
            .disabled(!enabled)
            .size(surface::css(30.))
            .flex_shrink_0()
            .ml(surface::css(20.))
            .p_0()
            .rounded_full()
            .when(!enabled, |s| s.opacity(0.3))
            .focus_visible(|s| s.border_1().border_color(cx.theme().ring))
            .bg(if selected {
                cx.theme().primary
            } else {
                cx.theme().background
            })
            .text_color(if selected {
                cx.theme().primary_foreground
            } else {
                cx.theme().foreground
            })
            .child(div().relative().size_full().when(enabled, |el| {
                el.child(
                    img(SharedString::from(format!("synapse/stage-{ordinal}.svg")))
                        .absolute()
                        .top(surface::css(3.))
                        .left(surface::css(11.))
                        .w(surface::css(8.))
                        .h(surface::css(6.)),
                )
                .child(
                    div()
                        .absolute()
                        .top(surface::css(10.))
                        .w_full()
                        .text_center()
                        .text_size(surface::css(14.))
                        .child(ordinal.to_string()),
                )
            }))
            .on_click(cx.listener(move |owner, _, w, cx| {
                owner.edit(w, cx, |s| {
                    if let Some(index) = s.sensitivity.editable_slot(id) {
                        s.sensitivity.select_stage(index);
                    }
                })
            }));

        div()
            .id(SharedString::from(format!("sensitivity-slot-{id}")))
            .test_support()
            .track_focus(&controls.focus)
            .on_hover(cx.listener(move |owner, hovered, _, cx| {
                let controls = owner.sensitivity_controls.slots.get_mut(&id).unwrap();
                if controls.hovered != *hovered {
                    controls.hovered = *hovered;
                    cx.notify();
                }
            }))
            .flex()
            .items_center()
            .h(surface::css(if independent { 114. } else { 68. }))
            .flex_shrink_0()
            .mb(surface::css(4.))
            .rounded(surface::css(3.))
            .focus_visible(|s| s.border_1().border_color(cx.theme().ring))
            .when(selected, |s| s.hover(|s| s.bg(cx.theme().background)))
            .when(state.visible, |s| {
                let device = self.identity();
                let profile = self.device().active_profile.clone();
                s.drag_over::<DpiDrag>(move |s, drag, _, cx| {
                    if drag.device != device || drag.profile != profile || drag.slot == id {
                        return s;
                    }
                    let style = if drag.position > index {
                        s.border_t_2()
                    } else {
                        s.border_b_2()
                    };
                    style.border_color(cx.theme().primary)
                })
            })
            .on_drop(cx.listener(move |owner, drag: &DpiDrag, w, cx| {
                if drag.device == owner.identity() && drag.profile == owner.device().active_profile
                {
                    owner.edit(w, cx, |s| s.sensitivity.move_slot(drag.slot, id));
                }
            }))
            .child(stage_button)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .ml(surface::css(10.))
                    .mb(surface::css(10.))
                    .gap(surface::css(10.))
                    .when(!enabled, |s| s.opacity(0.3))
                    .children((0..if independent { 2 } else { 1 }).map(|axis| {
                        div()
                            .flex()
                            .items_center()
                            .h(surface::css(27.))
                            .gap(surface::css(10.))
                            .child(DpiNumberInput {
                                id,
                                axis,
                                label: format!("{label} DPI {}", if axis == 0 { "X" } else { "Y" })
                                    .into(),
                                disabled: !enabled,
                                show_controls: show_actions,
                                input: controls.inputs[axis].clone(),
                                owner: cx.entity().downgrade(),
                            })
                            .child(
                                div()
                                    .id(SharedString::from(format!("dpi-slot-{id}-slider-{axis}")))
                                    .relative()
                                    .w(surface::css(250.))
                                    .child(Slider::new(&controls.sliders[axis]).disabled(!enabled))
                                    .when(independent, |s| {
                                        // Vm's thumbTag belongs to the thumb, so enabling XY
                                        // must not push the trailing controls out of the row.
                                        let fraction =
                                            (state.stages[index][axis] - 100) as f32 / 29900.;
                                        s.child(
                                            div()
                                                .absolute()
                                                .left(relative(fraction))
                                                .ml(surface::css(-8.))
                                                .top(surface::css(4.))
                                                .size(surface::css(16.))
                                                .line_height(surface::css(16.))
                                                .text_center()
                                                .text_size(surface::css(12.))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().primary_foreground)
                                                .child(if axis == 0 { "X" } else { "Y" }),
                                        )
                                    }),
                            )
                    })),
            )
            .child(
                div()
                    .size(surface::css(32.))
                    .flex_shrink_0()
                    .ml(surface::css(10.))
                    .when(show_actions, |s| {
                        s.child(
                            BaseButton::new(SharedString::from(format!("dpi-slot-{id}-xy")))
                                .accessibility_label(format!(
                                    "{label} {}",
                                    razer_i18n::t(if independent {
                                        "DISABLE_XY"
                                    } else {
                                        "ENABLE_XY"
                                    })
                                ))
                                .disabled(!enabled)
                                .size(surface::css(32.))
                                .flex_shrink_0()
                                .p_0()
                                .focus_visible(|s| s.border_1().border_color(cx.theme().ring))
                                .when(enabled, |s| s.hover(|s| s.opacity(0.7)))
                                .child(
                                    img(if !enabled {
                                        "synapse/sensitivity-xy-disabled.svg"
                                    } else if independent {
                                        "synapse/sensitivity-xy-active.svg"
                                    } else {
                                        "synapse/sensitivity-xy.svg"
                                    })
                                    .size_full(),
                                )
                                .on_click(cx.listener(move |owner, _, w, cx| {
                                    owner.edit(w, cx, |s| s.sensitivity.link_slot(id, !independent))
                                })),
                        )
                    }),
            )
            .when(state.visible, |s| {
                s.child(
                    div()
                        .flex_shrink_0()
                        .w(surface::css(32.))
                        .ml(surface::css(15.))
                        .when(show_actions, |s| {
                            s.child(
                                SynapseSwitch::new(SharedString::from(format!(
                                    "dpi-slot-{id}-enabled"
                                )))
                                .accessibility_label(format!("{label} 启用"))
                                .checked(enabled)
                                .disabled(enabled && state.enabled_count() <= 2)
                                .on_change(cx.listener(
                                    move |owner, next, w, cx| {
                                        owner.edit(w, cx, |s| s.sensitivity.set_enabled(id, *next))
                                    },
                                )),
                            )
                        }),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("dpi-slot-{id}-drag")))
                        .ml(surface::css(15.))
                        .w(surface::css(10.))
                        .h_full()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .when(!enabled, |s| s.opacity(0.3))
                        .when(show_actions, |s| {
                            s.cursor_move()
                                .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                                .on_key_down(cx.listener(
                                    move |owner, event: &KeyDownEvent, w, cx| {
                                        if !event.keystroke.modifiers.alt {
                                            return;
                                        }
                                        let delta = match event.keystroke.key.as_str() {
                                            "up" => -1,
                                            "down" => 1,
                                            _ => return,
                                        };
                                        let state = &owner.settings().sensitivity;
                                        let Some(from) =
                                            state.slots.iter().position(|s| s.id == id)
                                        else {
                                            return;
                                        };
                                        let to = (from as isize + delta)
                                            .clamp(0, state.slots.len() as isize - 1)
                                            as usize;
                                        let target = state.slots[to].id;
                                        owner.edit(w, cx, |s| s.sensitivity.move_slot(id, target));
                                        cx.stop_propagation();
                                    },
                                ))
                                .child(
                                    BaseButton::new(SharedString::from(format!(
                                        "dpi-slot-{id}-move"
                                    )))
                                    .accessibility_label(format!("移动{label}，Alt 加上下方向键"))
                                    .w(surface::css(10.))
                                    .h(surface::css(32.))
                                    .p_0()
                                    .focus_visible(|s| s.border_1().border_color(cx.theme().ring))
                                    .child(svg().path("synapse/dpi-draggable.svg").size_full()),
                                )
                        }),
                )
            })
            .into_any_element()
    }
}
