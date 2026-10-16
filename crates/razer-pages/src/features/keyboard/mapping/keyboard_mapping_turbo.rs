//! Current keyboard/mouse turbo controls, original 44230 Stepper and slider.
use super::*;
use razer_widgets::stepper::{Stepper, StepperEvent};

#[derive(Default)]
pub(super) struct State {
    speeds: [Option<Entity<Stepper>>; 2],
    sliders: [Option<Entity<SliderState>>; 2],
    subscriptions: [Vec<Subscription>; 2],
    last_speed: [Value; 2],
    syncing: bool,
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.speeds = Default::default();
        self.sliders = Default::default();
        self.subscriptions = Default::default();
        self.last_speed = Default::default();
    }
}
impl KeyboardProductWorkspace {
    fn mapping_turbo_group(&self, slot: usize) -> Option<&'static str> {
        let edit = self.analog_gamepad.mapping.edit.as_ref()?;
        let category = &edit.slots[slot].category;
        let group = match category.as_str() {
            "KEYBOARD_FUNCTION" if !edit.secondary_enabled => "keyboardGroup",
            "MOUSE_FUNCTION" => "mouseGroup",
            _ => return None,
        };
        if source(self.spec.product_id)
            .no_turbo
            .iter()
            .any(|id| edit.button["buttonKey"] == id.as_str())
            || edit.button["inputType"] == "ControllerInput"
            || edit.button["disableTurboInAssignment"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row == category))
            || edit.slots[slot].staged["mouseGroup"]["mouseAssignment"] == "ScrollModeSwitch"
        {
            return None;
        }
        Some(group)
    }
    pub(super) fn init_mapping_turbo(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(group) = self.mapping_turbo_group(slot) else {
            self.analog_gamepad.mapping.turbo.speeds[slot] = None;
            self.analog_gamepad.mapping.turbo.sliders[slot] = None;
            self.analog_gamepad.mapping.turbo.subscriptions[slot].clear();
            self.analog_gamepad.mapping.turbo.last_speed[slot] = Value::Null;
            return;
        };
        let value = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .and_then(|edit| edit.slots[slot].staged[group]["turboMode"]["keysPerSecond"].as_f64())
            .unwrap_or(7.);
        self.analog_gamepad.mapping.turbo.last_speed[slot] = json!(value);
        let speed = cx.new(|cx| {
            Stepper::new(
                SharedString::from(format!("keyboard-map-turbo-speed-{slot}")),
                value,
                (1., 20., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        let slider = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(20.)
                .step(1.)
                .default_value(value as f32)
        });
        let speed_subscription = cx.subscribe_in(
            &speed,
            window,
            move |this, _, event: &StepperEvent, window, cx| {
                if this.analog_gamepad.mapping.turbo.syncing
                    || this.mapping_turbo_group(slot) != Some(group)
                {
                    return;
                }
                let Some(edit) = &mut this.analog_gamepad.mapping.edit else {
                    return;
                };
                if edit.slots[slot].staged[group]["turboMode"]["isTurbo"] != true {
                    return;
                }
                let value = event
                    .draft
                    .as_ref()
                    .map(|draft| json!(draft))
                    .unwrap_or_else(|| json!(event.value as u32));
                edit.slots[slot].staged[group]["turboMode"]["keysPerSecond"] = value.clone();
                this.analog_gamepad.mapping.turbo.last_speed[slot] = value;
                if event.draft.is_none() {
                    this.sync_mapping_turbo(slot, window, cx);
                }
                cx.emit(KeyboardProductChanged);
                cx.notify();
            },
        );
        let slider_subscription =
            cx.subscribe_in(&slider, window, move |this, _, event, window, cx| {
                if this.analog_gamepad.mapping.turbo.syncing
                    || this.mapping_turbo_group(slot) != Some(group)
                {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                        if edit.slots[slot].staged[group]["turboMode"]["isTurbo"] != true {
                            return;
                        }
                        let speed = json!(value.start().round() as u32);
                        edit.slots[slot].staged[group]["turboMode"]["keysPerSecond"] =
                            speed.clone();
                        this.analog_gamepad.mapping.turbo.last_speed[slot] = speed;
                    }
                    this.sync_mapping_turbo(slot, window, cx);
                    cx.emit(KeyboardProductChanged);
                    cx.notify();
                }
            });
        self.analog_gamepad.mapping.turbo.speeds[slot] = Some(speed);
        self.analog_gamepad.mapping.turbo.sliders[slot] = Some(slider);
        self.analog_gamepad.mapping.turbo.subscriptions[slot] =
            vec![speed_subscription, slider_subscription];
        self.sync_mapping_turbo(slot, window, cx);
    }
    pub(super) fn sync_mapping_turbo(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let group = self.mapping_turbo_group(slot);
        let enabled = group.is_some_and(|group| {
            self.analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(|edit| edit.slots[slot].staged[group]["turboMode"]["isTurbo"] == true)
        });
        let speed = group
            .and_then(|group| {
                self.analog_gamepad.mapping.edit.as_ref().and_then(|edit| {
                    edit.slots[slot].staged[group]["turboMode"]["keysPerSecond"].as_f64()
                })
            })
            .or_else(|| self.analog_gamepad.mapping.turbo.last_speed[slot].as_f64())
            .unwrap_or(7.);
        self.analog_gamepad.mapping.turbo.syncing = true;
        if let Some(stepper) = self.analog_gamepad.mapping.turbo.speeds[slot].clone() {
            stepper.update(cx, |stepper, cx| {
                stepper.sync_value(speed, 1., !enabled, window, cx)
            });
        }
        if let Some(slider) = self.analog_gamepad.mapping.turbo.sliders[slot].clone() {
            slider.update(cx, |slider, cx| slider.set_value(speed as f32, window, cx));
        }
        self.analog_gamepad.mapping.turbo.syncing = false;
    }
    pub(super) fn mapping_turbo_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let group = self.mapping_turbo_group(slot)?;
        let edit = self.analog_gamepad.mapping.edit.as_ref()?;
        let enabled = edit.slots[slot].staged[group]["turboMode"]["isTurbo"] == true;
        let state = &self.analog_gamepad.mapping.turbo;
        let mut panel = v_flex()
            .mx(surface::css(20.))
            .mb(surface::css(20.))
            .gap(surface::css(5.));
        if slot == 0 || group == "mouseGroup" {
            panel = panel.child(
                h_flex()
                    .child(
                        Checkbox::new(SharedString::from(format!("keyboard-map-turbo-{slot}")))
                            .label(t("ENABLE_TURBO"))
                            .checked(enabled)
                            .on_click(cx.listener(move |this, enabled: &bool, window, cx| {
                                let speed =
                                    match &this.analog_gamepad.mapping.turbo.last_speed[slot] {
                                        Value::Null => json!(7),
                                        speed => speed.clone(),
                                    };
                                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                    if *enabled {
                                        edit.slots[slot].staged[group]["turboMode"] =
                                            json!({"isTurbo":true,"keysPerSecond":speed});
                                    } else if let Some(group) =
                                        edit.slots[slot].staged[group].as_object_mut()
                                    {
                                        group.remove("turboMode");
                                    }
                                }
                                this.sync_mapping_turbo(slot, window, cx);
                                cx.emit(KeyboardProductChanged);
                                cx.notify();
                            })),
                    )
                    .child(surface::help_control(
                        SharedString::from(format!("keyboard-map-turbo-help-{slot}")),
                        t("MAPPING_TURBO_CONTROL_TIP"),
                    )),
            );
        }
        if enabled {
            panel = panel
                .child(t("SPEED_PER_SEC"))
                .children(state.speeds[slot].clone())
                .children(state.sliders[slot].as_ref().map(|slider| {
                    h_flex()
                        .gap(surface::css(5.))
                        .child("1")
                        .child(Slider::new(slider).flex_1())
                        .child("20")
                }));
        }
        Some(panel.into_any_element())
    }
}
