//! Native controller pages from each current product's mounted components.
//! `profile` and the independent Redux controller states remain separate. These
//! are local drafts; service acknowledgements and live tester data are not faked.
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, StyledExt,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

use crate::{i18n::t, ui::surface};

#[derive(Deserialize)]
pub(crate) struct GamepadProductSpec {
    product_id: u32,
    name: String,
    profile: Value,
    controller: Value,
    pages: Vec<String>,
    button_controls: Vec<Value>,
    assignments: Vec<String>,
    arcade: bool,
    socd_modes: BTreeMap<String, String>,
    trigger_reset: Value,
    analog_mode: i64,
    digital_mode: i64,
    standard_mode: i64,
    circular_mode: i64,
    deadzone_steps: Vec<i64>,
    sensitivity_steps: Vec<i64>,
    power_minutes: Vec<i64>,
    brightness_step: Option<f32>,
    controller_lighting: bool,
    effects: Vec<Value>,
    polling_rates: Vec<i64>,
    info: Value,
}

pub(crate) fn source_product(pid: u32) -> Option<&'static GamepadProductSpec> {
    static PRODUCTS: OnceLock<Vec<GamepadProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("gamepad_products_data.json"))
                .expect("validated current gamepad specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}

pub(crate) struct GamepadProductChanged;

pub(crate) struct GamepadProductWorkspace {
    spec: &'static GamepadProductSpec,
    page: String,
    draft: Value,
    selected_button: Option<String>,
    sensitivity: bool,
    low_deadzone: Option<(String, Value)>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<GamepadProductChanged> for GamepadProductWorkspace {}

impl GamepadProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited gamepad product");
        let mut this = Self {
            spec,
            page: "TAB_CUSTOMIZE".into(),
            draft: json!({"profile": spec.profile, "controller": spec.controller}),
            selected_button: None,
            sensitivity: false,
            low_deadzone: None,
            sliders: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
        };
        if spec.pages.iter().any(|p| p == "TRIGGERS") {
            for side in ["leftTrigger", "rightTrigger"] {
                for (field, min) in [("startRange", 0.), ("endRange", 0.), ("actuationPoint", 1.)] {
                    this.add_slider(
                        &format!("/controller/{side}/{field}"),
                        min,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                }
            }
        }
        if let Some(step) = spec.brightness_step {
            this.add_slider("/profile/brightness/value", 0., 100., step, window, cx);
            if !spec.controller_lighting {
                this.add_slider(
                    "/profile/switchOffLighting/idleMinutes",
                    1.,
                    15.,
                    1.,
                    window,
                    cx,
                );
            }
        }
        this
    }

    pub(crate) fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.page = key.into();
            self.low_deadzone = None;
            cx.notify();
        }
    }

    pub(crate) fn snapshot(&self) -> Value {
        self.draft.clone()
    }

    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = json!({"profile": self.spec.profile, "controller": self.spec.controller});
        if let Some(saved) =
            saved.filter(|v| v["profile"].is_object() && v["controller"].is_object())
        {
            merge_known(&mut self.draft, saved);
        }
        self.selected_button = None;
        self.low_deadzone = None;
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn add_slider(
        &mut self,
        path: &str,
        min: f32,
        max: f32,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) else {
            return;
        };
        let state = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(value as f32)
        });
        let key = path.to_owned();
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let mut value = value.start().clamp(min, max).round() as i64;
                    // The source's dual range control cannot cross its other handle.
                    if key.ends_with("/startRange") {
                        let other = key.replace("/startRange", "/endRange");
                        value = value.min(this.number(&other));
                    } else if key.ends_with("/endRange") {
                        let other = key.replace("/endRange", "/startRange");
                        value = value.max(this.number(&other));
                    }
                    this.write(&key, json!(value), cx);
                    this.sync_sliders(window, cx);
                }
            },
        ));
        self.sliders.insert(path.into(), state);
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            }
        }
        self.syncing = false;
    }

    fn write(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        if let Some(target) = self.draft.pointer_mut(path) {
            if *target != value {
                *target = value;
                cx.emit(GamepadProductChanged);
                cx.notify();
            }
        }
    }

    fn number(&self, path: &str) -> i64 {
        self.draft
            .pointer(path)
            .and_then(Value::as_i64)
            .expect("audited numeric controller field")
    }

    fn checked(&self, path: &str) -> bool {
        self.draft
            .pointer(path)
            .and_then(Value::as_bool)
            .expect("audited boolean controller field")
    }

    fn toggle(&self, path: &str, label: String, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let path = path.to_owned();
        Checkbox::new(SharedString::from(format!("gamepad-{path}")))
            .label(label)
            .checked(self.checked(&path))
            .disabled(!enabled)
            .on_click(cx.listener(move |this, value, _, cx| {
                if enabled {
                    this.write(&path, json!(value), cx);
                }
            }))
            .into_any_element()
    }

    fn choice(
        &self,
        path: &str,
        value: Value,
        label: String,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Button {
        let path = path.to_owned();
        Button::new(SharedString::from(format!("gamepad-{path}-{value}")))
            .label(label)
            .outline()
            .disabled(!enabled)
            .selected(self.draft.pointer(&path) == Some(&value))
            .on_click(cx.listener(move |this, _, _, cx| {
                if enabled {
                    this.write(&path, value.clone(), cx);
                }
            }))
    }

    fn range(&self, path: &str, label: String, enabled: bool) -> AnyElement {
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .child(label)
                    .child(format!("{}%", self.number(path))),
            )
            .child(Slider::new(slider).disabled(!enabled))
            .into_any_element()
    }

    fn triggers(&self, cx: &Context<Self>) -> AnyElement {
        let panels = [
            ("leftTrigger", "LEFT_TRIGGER"),
            ("rightTrigger", "RIGHT_TRIGGER"),
        ]
        .map(|(side, label)| {
            let root = format!("/controller/{side}");
            let analog = self.number(&format!("{root}/mode")) == self.spec.analog_mode;
            let reset_fields: &[&str] = if analog {
                &["startRange", "endRange"]
            } else {
                &["actuationPoint", "isRapidTrigger"]
            };
            let changed = reset_fields.iter().any(|field| {
                self.draft.pointer(&format!("{root}/{field}")) != self.spec.trigger_reset.get(field)
            });
            let mut panel = surface::panel(t(label), cx).child(
                h_flex()
                    .gap_2()
                    .child(self.choice(
                        &format!("{root}/mode"),
                        json!(self.spec.analog_mode),
                        t("ANALOG"),
                        true,
                        cx,
                    ))
                    .child(self.choice(
                        &format!("{root}/mode"),
                        json!(self.spec.digital_mode),
                        t("DIGITAL"),
                        true,
                        cx,
                    )),
            );
            if analog {
                panel = panel
                    .child(self.range(&format!("{root}/startRange"), t("MINIMUM"), true))
                    .child(self.range(&format!("{root}/endRange"), t("MAXIMUM"), true));
            } else {
                panel = panel
                    .child(self.range(
                        &format!("{root}/actuationPoint"),
                        t("ACTUATION_POINT"),
                        true,
                    ))
                    .child(self.toggle(
                        &format!("{root}/isRapidTrigger"),
                        t("RAPID_TRIGGER"),
                        true,
                        cx,
                    ));
            }
            panel.child(
                Button::new(SharedString::from(format!("gamepad-reset-{side}")))
                    .label(t("RESET"))
                    .outline()
                    .disabled(!changed)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let fields: &[&str] = if analog {
                            &["startRange", "endRange"]
                        } else {
                            &["actuationPoint", "isRapidTrigger"]
                        };
                        for field in fields {
                            this.write(
                                &format!("/controller/{side}/{field}"),
                                this.spec.trigger_reset[*field].clone(),
                                cx,
                            );
                        }
                        this.sync_sliders(window, cx);
                    })),
            )
        });
        v_flex()
            .gap_5()
            .child(surface::note(t("ACTUATION_DESC"), cx))
            .child(surface::page_columns().children(panels.into_iter().map(surface::page_column)))
            .when_some(
                self.spec.info["minFWSupportTriggers"].as_str(),
                |v, version| {
                    v.child(surface::note(
                        format!("此产品扳机功能要求固件 {version}；当前未读取设备固件。"),
                        cx,
                    ))
                },
            )
            .into_any_element()
    }

    fn set_mapping(&mut self, input: &str, assignment: Option<&str>, cx: &mut Context<Self>) {
        let Some(button) = self
            .spec
            .button_controls
            .iter()
            .find(|b| b["inputID"].as_str() == Some(input))
        else {
            return;
        };
        let Some(mappings) = self
            .draft
            .pointer_mut("/profile/mappings")
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        mappings.retain(|m| m["inputID"].as_str() != Some(input));
        if let Some(assignment) = assignment {
            let mut mapping = json!({"inputID":input,"inputType":button["inputType"],"controllerInput":input,"isHyperShift":false});
            if assignment == "DISABLE" {
                mapping["outputType"] = json!("disableGroup");
                mapping["disableGroup"] = json!({});
            } else {
                mapping["outputType"] = json!("controllerGroup");
                mapping["controllerGroup"] = json!({"controllerAssignment":assignment});
            }
            mappings.push(mapping);
        }
        cx.emit(GamepadProductChanged);
        cx.notify();
    }

    fn customize(&self, cx: &Context<Self>) -> AnyElement {
        let mut left = surface::panel(t("TAB_CUSTOMIZE"), cx).child(self.spec.name.clone());
        if !self.spec.arcade {
            left = left.child(h_flex().gap_2().flex_wrap().children(
                self.spec.button_controls.iter().filter_map(|button| {
                    let id = button["inputID"].as_str()?.to_owned();
                    let label = button["counter"].as_str().unwrap_or(&id).to_owned();
                    Some(
                        Button::new(SharedString::from(format!("gamepad-input-{id}")))
                            .label(label)
                            .outline()
                            .selected(self.selected_button.as_ref() == Some(&id))
                            .disabled(button["isEnabled"].as_bool() == Some(false))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_button = Some(id.clone());
                                cx.notify();
                            })),
                    )
                }),
            ));
            if let Some(input) = &self.selected_button {
                if let Some(button) = self
                    .spec
                    .button_controls
                    .iter()
                    .find(|b| b["inputID"].as_str() == Some(input))
                {
                    let supported = |key: &str| {
                        button["functionList"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(key)))
                    };
                    let current = self.draft["profile"]["mappings"]
                        .as_array()
                        .and_then(|a| a.iter().find(|m| m["inputID"].as_str() == Some(input)));
                    let selected =
                        current.and_then(|m| m["controllerGroup"]["controllerAssignment"].as_str());
                    left = left.child(div().font_bold().child(input.clone()));
                    if supported("DEFAULT") {
                        let input = input.clone();
                        left = left.child(
                            Button::new("gamepad-mapping-default")
                                .label(t("DEFAULT"))
                                .outline()
                                .selected(current.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_mapping(&input, None, cx)
                                })),
                        );
                    }
                    if supported("CONTROLLER_V2") || supported("CONTROLLER_PLAYSTATION") {
                        left =
                            left.child(h_flex().gap_2().flex_wrap().children(
                                self.spec.assignments.iter().map(|assignment| {
                                    let input = input.clone();
                                    let assignment = assignment.clone();
                                    Button::new(SharedString::from(format!(
                                        "gamepad-assign-{assignment}"
                                    )))
                                    .label(t(&assignment))
                                    .outline()
                                    .selected(selected == Some(assignment.as_str()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_mapping(&input, Some(&assignment), cx)
                                    }))
                                }),
                            ));
                    }
                    if supported("DISABLE") {
                        let input = input.clone();
                        left = left.child(
                            Button::new("gamepad-mapping-disable")
                                .label(t("DISABLE"))
                                .outline()
                                .selected(
                                    current.is_some_and(|m| m["outputType"] == "disableGroup"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_mapping(&input, Some("DISABLE"), cx)
                                })),
                        );
                    }
                }
            }
        }
        let mut right = v_flex().gap_5().child(
            surface::panel(t("POLLING_RATE"), cx).child(h_flex().gap_2().flex_wrap().children(
                self.spec.polling_rates.iter().map(|rate| {
                    self.choice(
                        "/profile/pollingRate",
                        json!(rate),
                        format!("{rate} Hz"),
                        true,
                        cx,
                    )
                }),
            )),
        );
        if self.spec.arcade {
            right = right.child(
                surface::panel(t("MODE_SWITCHER"), cx)
                    .child(self.choice(
                        "/controller/modeSwitcher/mode",
                        json!("safe"),
                        t("MODE_SWITCHER_SAFE"),
                        true,
                        cx,
                    ))
                    .child(surface::note(t("MODE_SWITCHER_SAFE_DESC"), cx))
                    .child(self.choice(
                        "/controller/modeSwitcher/mode",
                        json!("standard"),
                        t("MODE_SWITCHER_STANDARD"),
                        true,
                        cx,
                    ))
                    .child(surface::note(t("MODE_SWITCHER_STANDARD_DESC"), cx)),
            );
            let mut socd = surface::panel(t("DPAD_SOCD_SETTINGS"), cx)
                .child(surface::note(t("DPAD_SOCD_SETTINGS_DESC"), cx));
            for (key, label) in [
                ("NEUTRAL", "NEUTRAL_SOCD"),
                ("FIRST", "FIRST_INPUT_SOCD"),
                ("LAST", "LAST_INPUT_SOCD"),
                ("ABSOLUTE_UP", "ABSOLUTE_UP_SOCD"),
                ("NONE", "NO_SOCD"),
            ] {
                if let Some(value) = self.spec.socd_modes.get(key) {
                    socd = socd.child(self.choice(
                        "/controller/dpad/socdSettings/value",
                        json!(value),
                        t(label),
                        true,
                        cx,
                    ));
                }
            }
            right = right.child(socd);
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }

    fn clutch_assigned(&self, side: &str) -> bool {
        self.draft["profile"]["mappings"]
            .as_array()
            .is_some_and(|mappings| {
                mappings.iter().any(|m| {
                    let assignment = m["controllerGroup"]["controllerAssignment"].as_str();
                    assignment == Some("GLOBAL_SENSITIVITY_CLUTCH")
                        || assignment
                            == Some(if side == "leftStick" {
                                "LEFT_SENSITIVITY_CLUTCH"
                            } else {
                                "RIGHT_SENSITIVITY_CLUTCH"
                            })
                })
            })
    }

    fn thumbsticks(&self, cx: &Context<Self>) -> AnyElement {
        let mut page = v_flex().gap_5().child(h_flex().gap_2().children(
            [(false, "DEADZONE"), (true, "SENSITIVITY_CLUTCH")].map(|(sensitivity, label)| {
                Button::new(SharedString::from(format!("gamepad-stick-tab-{label}")))
                    .label(t(label))
                    .outline()
                    .selected(self.sensitivity == sensitivity)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sensitivity = sensitivity;
                        cx.notify();
                    }))
            }),
        ));
        let field = if self.sensitivity {
            "sensitivityValue"
        } else {
            "deadzoneValue"
        };
        let steps = if self.sensitivity {
            &self.spec.sensitivity_steps
        } else {
            &self.spec.deadzone_steps
        };
        let panels = [
            ("leftStick", "LEFT_THUMBSTICK"),
            ("rightStick", "RIGHT_THUMBSTICK"),
        ]
        .map(|(side, label)| {
            let path = format!("/controller/{side}/{field}");
            let enabled = !self.sensitivity || self.clutch_assigned(side);
            surface::panel(t(label), cx)
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .children(steps.iter().map(|value| {
                            let path = path.clone();
                            let value = *value;
                            Button::new(SharedString::from(format!("gamepad-{path}-{value}")))
                                .label(format!("{value}%"))
                                .outline()
                                .disabled(!enabled)
                                .selected(self.number(&path) == value)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !enabled {
                                        return;
                                    }
                                    if !this.sensitivity && value < 7 && this.number(&path) != value
                                    {
                                        this.low_deadzone = Some((
                                            path.clone(),
                                            this.draft
                                                .pointer(&path)
                                                .cloned()
                                                .expect("source deadzone"),
                                        ));
                                    }
                                    this.write(&path, json!(value), cx);
                                }))
                        })),
                )
                .when(!enabled, |p| {
                    p.child(surface::note(t("SENSITIVITY_ASSIGN_INFO"), cx))
                })
        });
        page = page
            .child(surface::page_columns().children(panels.into_iter().map(surface::page_column)));
        if !self.sensitivity {
            page = page.child(
                surface::page_columns()
                    .child(surface::page_column(
                        surface::panel(t("CIRCULARITY_MODE_HEADER"), cx)
                            .child(self.choice(
                                "/controller/circularity/mode",
                                json!(self.spec.standard_mode),
                                t("STANDARD"),
                                true,
                                cx,
                            ))
                            .child(self.choice(
                                "/controller/circularity/mode",
                                json!(self.spec.circular_mode),
                                t("CIRCULAR"),
                                true,
                                cx,
                            )),
                    ))
                    .child(surface::page_column(
                        surface::panel(t("PREVENT_DOUBLE_DEADZONES"), cx)
                            .child(
                                Checkbox::new("gamepad-prevent-double-deadzone")
                                    .label(t("PREVENT_DOUBLE_DEADZONES"))
                                    .checked(
                                        self.checked("/controller/leftStick/preventDoubleDeadzone")
                                            && self.checked(
                                                "/controller/rightStick/preventDoubleDeadzone",
                                            ),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        for side in ["leftStick", "rightStick"] {
                                            let path =
                                                format!("/controller/{side}/preventDoubleDeadzone");
                                            this.write(&path, json!(!this.checked(&path)), cx);
                                        }
                                    })),
                            )
                            .child(surface::note(t("PREVENT_DOUBLE_DEADZONES_DESCRIPTION"), cx)),
                    )),
            );
        }
        if let Some((path, previous)) = &self.low_deadzone {
            let path = path.clone();
            let previous = previous.clone();
            page = page.child(
                surface::panel(t("LOW_DEADZONE_INFO_TITLE"), cx)
                    .child(surface::note(t("LOW_DEADZONE_INFO_DETAIL"), cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("gamepad-low-deadzone-continue")
                                    .label(t("CONTINUE"))
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.low_deadzone = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("gamepad-low-deadzone-revert")
                                    .label(t("CANCEL"))
                                    .outline()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.write(&path, previous.clone(), cx);
                                        this.low_deadzone = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        page.into_any_element()
    }

    fn lighting(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.checked("/profile/brightness/isEnabled");
        let mut left = surface::panel(t("BRIGHTNESS_HEADER"), cx)
            .child(self.toggle("/profile/brightness/isEnabled", t("BRIGHTNESS"), true, cx))
            .child(self.range("/profile/brightness/value", t("BRIGHTNESS"), enabled));
        if !self.spec.controller_lighting {
            left = left
                .child(self.toggle(
                    "/profile/switchOffLighting/isDisplayOn",
                    t("DISPLAY_TURNED_OFF"),
                    enabled,
                    cx,
                ))
                .child(self.toggle(
                    "/profile/switchOffLighting/isIdleEnabled",
                    t("IDLE_FOR_MIN"),
                    enabled,
                    cx,
                ))
                .child(self.range(
                    "/profile/switchOffLighting/idleMinutes",
                    t("MINUTES"),
                    enabled && self.checked("/profile/switchOffLighting/isIdleEnabled"),
                ));
        }
        let path = if self.spec.controller_lighting {
            "/controller/lighting/effectId"
        } else {
            "/profile/quickEffects/selectedEffectId"
        };
        let right = surface::panel(t("EFFECTS"), cx).children(self.spec.effects.iter().filter_map(
            |effect| {
                Some(self.choice(
                    path,
                    effect["id"].clone(),
                    t(effect["name"].as_str()?),
                    true,
                    cx,
                ))
            },
        ));
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }

    fn power(&self, cx: &Context<Self>) -> AnyElement {
        // Mounted gamepad pages pass noSwitch=true; the shared keyboard power
        // widget's switch must not be exposed here.
        surface::panel(t("KEYBOARD_POWER_SAVING_TITLE"), cx)
            .child(
                h_flex()
                    .gap_2()
                    .children(self.spec.power_minutes.iter().map(|value| {
                        self.choice(
                            "/controller/power/powerSaving/value",
                            json!(value),
                            format!("{value} {}", t("MINUTES")),
                            self.checked("/controller/power/powerSaving/isEnabled"),
                            cx,
                        )
                    })),
            )
            .into_any_element()
    }

    fn calibration(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel(t("TAB_CALIBRATION"), cx)
            .child(surface::note(t("CALIBRATION_STEP0"), cx))
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("gamepad-calibrate-left")
                            .label(t("CALIBBRTION_LEFT_THUMBSITCK"))
                            .outline()
                            .disabled(true),
                    )
                    .child(
                        Button::new("gamepad-calibrate-right")
                            .label(t("CALIBBRTION_RIGHT_THUMBSITCK"))
                            .outline()
                            .disabled(true),
                    ),
            )
            .child(surface::note(
                "未连接手柄校准服务，无法开始校准或读取摇杆位置。",
                cx,
            ))
            .into_any_element()
    }
}

impl Render for GamepadProductWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page.as_str() {
            "TAB_CUSTOMIZE" => self.customize(cx),
            "TRIGGERS" => self.triggers(cx),
            "THUMBSTICKS" => self.thumbsticks(cx),
            "TAB_LIGHTING" => self.lighting(cx),
            "TAB_POWER" => self.power(cx),
            "TAB_CALIBRATION" => self.calibration(cx),
            _ => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        };
        v_flex()
            .min_w_0()
            .gap_5()
            .p_5()
            .text_color(cx.theme().foreground)
            .child(content)
            .child(surface::note("设置保存在本地配置中；尚未写入手柄。", cx))
    }
}

/// Restore fields with an audited schema. Hardware action logs never become
/// controller acknowledgements merely because a local profile contains them.
fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if matches!(
                    key.as_str(),
                    "actionsFromUI" | "actionsFromLocalStorage" | "errorActions"
                ) {
                    continue;
                }
                if let Some(saved) = saved.get(key) {
                    merge_known(value, saved);
                }
            }
        }
        (target @ Value::Array(_), Value::Array(_))
        | (target @ Value::Bool(_), Value::Bool(_))
        | (target @ Value::Number(_), Value::Number(_))
        | (target @ Value::String(_), Value::String(_)) => *target = saved.clone(),
        _ => {}
    }
}
