use super::*;
use crate::features::lighting_color::LightingColorPicker;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::slider::SliderState;
use razer_widgets::stepper::Stepper;
use razer_widgets::stepper::StepperEvent;
use std::{cell::Cell, rc::Rc};
#[path = "delete_confirmation.rs"]
mod delete_confirmation;
use delete_confirmation::DeleteConfirmation;
#[path = "effects.rs"]
mod effect_parameters;
#[path = "quick_macro.rs"]
mod quick_macro;
use quick_macro::{QuickMacroEditor, QuickMacroSaved};
pub(super) enum EditorEvent {
    Save(Rule),
    Delete(u32),
}
impl EventEmitter<EditorEvent> for AutomationEditor {}
pub(super) struct AutomationEditor {
    draft: Rule,
    original: Rule,
    editing: bool,
    available: Vec<u32>,
    category: Entity<SelectState<Vec<Choice>>>,
    effects: [Entity<SelectState<Vec<Choice>>>; 2],
    colors: [[Entity<ColorPickerState>; 2]; 2],
    durations: [Entity<SliderState>; 2],
    duration_focus: [FocusHandle; 2],
    boosts: [Entity<Stepper>; 2],
    catalogs: BTreeMap<u32, Vec<Value>>,
    installed: Option<bool>,
    busy: bool,
    preview: bool,
    delete_confirmation: Option<Entity<DeleteConfirmation>>,
    delete_subscription: Option<Subscription>,
    delete_trigger_focus: FocusHandle,
    delete_trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
    delete_footer_bounds: Rc<Cell<Bounds<Pixels>>>,
    delete_hovered: bool,
    macro_picker: Option<bool>,
    alert: Option<String>,
    syncing: bool,
    subscriptions: Vec<Subscription>,
}
impl AutomationEditor {
    pub(super) fn new(
        rule: Rule,
        editing: bool,
        available: Vec<u32>,
        catalogs: BTreeMap<u32, Vec<Value>>,
        installed: Option<bool>,
        busy: bool,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let category = cx.new(|cx| SelectState::new(Self::choices(&available), None, window, cx));
        let effects = [false, true]
            .map(|_| cx.new(|cx| SelectState::new(Self::effect_choices(), None, window, cx)));
        let colors = std::array::from_fn(|_| {
            std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx)))
        });
        let durations = std::array::from_fn(|_| {
            cx.new(|_| {
                SliderState::new()
                    .min(1.)
                    .max(3.)
                    .step(1.)
                    .default_value(2.)
            })
        });
        let boosts = std::array::from_fn(|lane| {
            cx.new(|cx| {
                Stepper::new(
                    format!("automation-color-boost-{lane}"),
                    1.,
                    (0.25, 4., 0.25),
                    true,
                    true,
                    Some(4),
                    window,
                    cx,
                )
                .in_modes_area()
            })
        });
        let mut this = Self {
            draft: rule.clone(),
            original: rule,
            editing,
            available,
            category: category.clone(),
            effects,
            colors,
            durations,
            duration_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            boosts,
            catalogs,
            installed,
            busy,
            preview,
            delete_confirmation: None,
            delete_subscription: None,
            delete_trigger_focus: cx.focus_handle().tab_stop(true),
            delete_trigger_bounds: Rc::new(Cell::new(Bounds::default())),
            delete_footer_bounds: Rc::new(Cell::new(Bounds::default())),
            delete_hovered: false,
            macro_picker: None,
            alert: None,
            syncing: false,
            subscriptions: vec![],
        };
        this.subscriptions.push(cx.subscribe_in(
            &category,
            window,
            |this, _, event, window, cx| {
                if !this.syncing && !this.editing {
                    if let SelectEvent::Confirm(Some(id)) = event {
                        if let Ok(id) = id.parse::<u32>() {
                            this.draft = Rule::new(id);
                            this.original = this.draft.clone();
                            this.sync(window, cx);
                            cx.notify();
                        }
                    }
                }
            },
        ));
        for down in [false, true] {
            this.subscriptions.push(cx.subscribe_in(
                &this.effects[down as usize],
                window,
                move |this, _, event, window, cx| {
                    if !this.syncing {
                        if let SelectEvent::Confirm(Some(id)) = event {
                            if let Ok(id) = id.parse::<u32>() {
                                let mut value = Self::quick(id);
                                // LP restores the saved lane setting when that
                                // effect is selected again; unsaved alternatives
                                // are not an additional per-effect cache.
                                if let Some(saved) = this
                                    .original
                                    .lane(down)
                                    .data
                                    .iter()
                                    .find(|value| {
                                        value["selectedEffectId"].as_u64() == Some(id.into())
                                    })
                                    .and_then(|value| value.get("setting"))
                                    .filter(|setting| setting.is_object())
                                {
                                    value["setting"] = saved.clone();
                                    value["setting"]["effectId"] = json!(id);
                                }
                                this.choose(down, value, cx);
                                this.sync(window, cx);
                            }
                        }
                    }
                },
            ));
        }
        for down in [false, true] {
            for channel in 0..2 {
                this.subscriptions.push(cx.subscribe_in(
                    &this.colors[down as usize][channel],
                    window,
                    move |this, _, event, _, cx| {
                        if this.syncing || !this.chroma_parameters_editable(down) {
                            return;
                        }
                        let value = this
                            .draft
                            .lane(down)
                            .data
                            .first()
                            .cloned()
                            .unwrap_or_default();
                        let effect = value["selectedEffectId"].as_u64().unwrap_or(4);
                        if !matches!(effect, 1 | 2 | 7)
                            || (effect == 1 && channel == 1)
                            || value["setting"]["isRandom"] == true
                        {
                            return;
                        }
                        let ColorPickerEvent::Change(color) = event;
                        if effect == 1 && color.is_none() {
                            return;
                        }
                        let color = color
                            .map(|color| {
                                let color = Rgba::from(color);
                                format!(
                                    "#{:02x}{:02x}{:02x}",
                                    (color.r * 255.).round() as u8,
                                    (color.g * 255.).round() as u8,
                                    (color.b * 255.).round() as u8
                                )
                            })
                            .unwrap_or_else(|| "no-color".into());
                        this.update_chroma_setting(
                            down,
                            if channel == 0 { "color1" } else { "color2" },
                            json!(color),
                            cx,
                        );
                    },
                ));
            }
        }
        for down in [false, true] {
            this.subscriptions.push(cx.observe_in(
                &this.durations[down as usize],
                window,
                move |this, slider, window, cx| {
                    if this.syncing || this.selected_effect(down) != 7 {
                        return;
                    }
                    let saved = this
                        .draft
                        .lane(down)
                        .data
                        .first()
                        .and_then(|v| v["setting"]["duration"].as_f64())
                        .unwrap_or(2.) as f32;
                    let raw = slider.read(cx).value().start();
                    let editable = this.chroma_parameters_editable(down);
                    let value = if editable {
                        raw.round().clamp(1., 3.)
                    } else {
                        saved
                    };
                    // Observe also covers Base Slider's accessibility actions,
                    // which set_value without emitting Change. Equality checks
                    // keep parent synchronization from clearing apply flags.
                    if raw != value
                        || (slider.read(cx).percentage().end - (value - 1.) / 2.).abs()
                            > f32::EPSILON
                    {
                        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                    }
                    if editable && saved != value {
                        this.update_chroma_setting(down, "duration", json!(value as u32), cx);
                    }
                },
            ));
            this.subscriptions.push(cx.subscribe_in(
                &this.boosts[down as usize],
                window,
                move |this, stepper, event: &StepperEvent, window, cx| {
                    if this.syncing
                        || !this.chroma_parameters_editable(down)
                        || this.selected_effect(down) != 12
                    {
                        return;
                    }
                    let boost = (event.value * 4.).ceil().clamp(1., 16.) / 4.;
                    let Some(value) = this.draft.lane(down).data.first() else {
                        return;
                    };
                    if value["setting"]["colorBoost"].as_f64() != Some(boost) {
                        // AI.changeColorBoost replaces settings, unlike Wave/Starlight.
                        this.choose(
                            down,
                            Self::quick_payload(12, json!({"colorBoost":boost})),
                            cx,
                        );
                    }
                    stepper.update(cx, |stepper, cx| {
                        stepper.sync_value(boost, 0.25, false, window, cx)
                    });
                },
            ));
        }
        this.sync(window, cx);
        this
    }

    fn open_macro_picker(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let names = self
            .catalogs
            .get(&5)
            .into_iter()
            .flatten()
            .filter_map(|value| value["name"].as_str().map(str::to_owned))
            .collect();
        let picker = cx.new(|cx| QuickMacroEditor::new(names, window, cx));
        self.subscriptions.push(cx.subscribe(
            &picker,
            move |this, _, event: &QuickMacroSaved, cx| {
                this.catalogs.entry(5).or_default().push(event.0.clone());
                this.choose(down, event.0.clone(), cx);
                this.macro_picker = None;
                cx.notify();
            },
        ));
        self.macro_picker = Some(down);
        let rem_size = window.rem_size();
        let margin_top = (window.viewport_size().height - rem_size * (369. / 16.)) / 2.;
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(rem_size * (500. / 16.))
                .margin_top(margin_top)
                .overlay(false)
                .overlay_closable(true)
                // QuickMacroEditor owns Escape while recording a keyboard chord.
                .keyboard(false)
                .close_button(false)
                .p_0()
                .bg(Colors::panel())
                .rounded(surface::css(3.))
                .child(picker.clone())
        });
    }
    fn choices(ids: &[u32]) -> Vec<Choice> {
        ids.iter()
            .map(|id| Choice::new(id.to_string(), text(&spec().actions[*id as usize].content)))
            .collect()
    }
    fn effect_choices() -> Vec<Choice> {
        spec()
            .effects
            .iter()
            .map(|e| Choice::new(e.id.to_string(), text(&e.name)))
            .collect()
    }
    fn quick(id: u32) -> Value {
        let mut setting = spec()
            .effect_defaults
            .get(&id.to_string())
            .cloned()
            .unwrap_or_else(|| json!({}));
        setting["effectId"] = json!(id);
        if id == 4 {
            setting["direction"] = json!(4);
        }
        Self::quick_payload(id, setting)
    }
    fn quick_payload(id: u32, setting: Value) -> Value {
        json!({"selectedEffectId":id,"setting":setting,"isApplyToOtherDevices":false,"isAdvEffect":false,"index":spec().effects.iter().position(|e|e.id==id).unwrap_or(0)})
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        self.category.update(cx, |s, cx| {
            s.set_selected_value(&self.draft.id.to_string(), window, cx)
        });
        for down in [false, true] {
            let id = self
                .draft
                .lane(down)
                .data
                .first()
                .and_then(|v| v["selectedEffectId"].as_u64())
                .unwrap_or(4);
            self.effects[down as usize].update(cx, |s, cx| {
                s.set_selected_value(&id.to_string(), window, cx)
            });
            let value = self
                .draft
                .lane(down)
                .data
                .first()
                .cloned()
                .unwrap_or_else(|| Self::quick(id as u32));
            self.durations[down as usize].update(cx, |slider, cx| {
                slider.set_value(
                    value["setting"]["duration"].as_f64().unwrap_or(2.) as f32,
                    window,
                    cx,
                )
            });
            let disabled = !self.chroma_parameters_editable(down) || id != 12;
            self.boosts[down as usize].update(cx, |stepper, cx| {
                stepper.sync_value(
                    value["setting"]["colorBoost"].as_f64().unwrap_or(1.),
                    0.25,
                    disabled,
                    window,
                    cx,
                )
            });
            for channel in 0..2 {
                let color = value["setting"]
                    .get(if channel == 0 { "color1" } else { "color2" })
                    .and_then(Value::as_str)
                    .and_then(|color| color.strip_prefix('#'))
                    .filter(|color| color.len() == 6)
                    .and_then(|color| u32::from_str_radix(color, 16).ok());
                self.colors[down as usize][channel].update(cx, |picker, cx| {
                    if let Some(color) = color {
                        picker.set_value(rgb(color), window, cx);
                    } else {
                        picker.clear_value(window, cx);
                    }
                });
            }
        }
        self.syncing = false;
    }
    fn choose(&mut self, down: bool, value: Value, cx: &mut Context<Self>) {
        if !self.draft.lane(down).is_enabled || (self.busy && self.draft.id == 0) {
            return;
        }
        self.draft.lane_mut(down).data = vec![value];
        cx.notify();
    }
    fn chroma_parameters_editable(&self, down: bool) -> bool {
        self.draft.id == 0
            && self.draft.lane(down).is_enabled
            && !self.busy
            && self
                .draft
                .lane(down)
                .data
                .first()
                .is_some_and(|value| value["isOff"] != true && value["isAdvEffect"] != true)
    }
    fn update_chroma_setting(
        &mut self,
        down: bool,
        key: &str,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        if !self.chroma_parameters_editable(down) {
            return;
        }
        let Some(mut data) = self.draft.lane(down).data.first().cloned() else {
            return;
        };
        if !data["setting"].is_object() {
            data["setting"] = json!({});
        }
        data["setting"][key] = value;
        // LP's P callback emits the updated setting for this lane and clears
        // apply-to-other-devices; it never updates the opposite trigger lane.
        let id = data["selectedEffectId"].as_u64().unwrap_or(4) as u32;
        self.choose(down, Self::quick_payload(id, data["setting"].take()), cx);
    }
    fn render_color_parameters(
        &self,
        down: bool,
        value: &Value,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let effect = value["selectedEffectId"].as_u64().unwrap_or(4);
        if !matches!(effect, 1 | 2 | 7) {
            return div().into_any_element();
        }
        let two_colors = matches!(effect, 2 | 7);
        let random = two_colors && value["setting"]["isRandom"] == true;
        h_flex()
            .items_start()
            .flex_wrap()
            .mt(surface::css(20.))
            .children((0..if two_colors { 2 } else { 1 }).map(|channel| {
                let label = if two_colors {
                    text("COLOR_DROP_NAME").replace("{{num}}", &(channel + 1).to_string())
                } else {
                    text("COLOR")
                };
                v_flex()
                    .mr(surface::css(20.))
                    .child(div().line_height(surface::css(20.)).child(label.clone()))
                    .child(
                        div().mt(surface::css(5.)).child(
                            LightingColorPicker::new(&self.colors[down as usize][channel], label)
                                .allow_none(two_colors)
                                .disabled(disabled || random),
                        ),
                    )
                    .into_any_element()
            }))
            .when(two_colors, |view| {
                view.child(
                    div().mt(surface::css(25.)).child(
                        checkbox::Checkbox::new(SharedString::from(format!(
                            "automation-random-{down}"
                        )))
                        .label(text("RANDOM_COLOR"))
                        .checked(random)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, on, _, cx| {
                            this.update_chroma_setting(down, "isRandom", json!(on), cx)
                        })),
                    ),
                )
            })
            .into_any_element()
    }
    fn enable(&mut self, down: bool, value: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.lane_mut(down).is_enabled = value;
        if value && self.draft.lane(down).data.is_empty() {
            let initial = match self.draft.id {
                0 => Some(Self::quick(4)),
                1 | 2 if down => Some(json!({"id":"restore"})),
                4 if down => Some(json!({"pauseGame":true})),
                kind => self.catalogs.get(&kind).and_then(|c| c.first()).cloned(),
            };
            if let Some(value) = initial {
                self.draft.lane_mut(down).data = vec![value];
            }
        }
        self.sync(window, cx);
        cx.notify();
    }
    fn request(&mut self, key: &str, cx: &mut Context<Self>) {
        self.alert = Some(if self.preview {
            format!("Preview request: {}", text(key))
        } else {
            format!(
                "{} is unavailable while its service is disconnected.",
                text(key)
            )
        });
        cx.notify();
    }
    fn launch_key(&self) -> &'static str {
        match self.draft.id {
            0 if self.installed == Some(true) => "LAUNCH_CHROMA_APP",
            0 => "INSTALL_CHROMA_APP",
            1 | 2 => "SOUND_SETTINGS",
            3 => "LAUNCH_GLOBAL_SHORTCUT",
            4 => "LAUNCH_LINKED_GAMES",
            _ => "LAUNCH_MACRO_MODULE",
        }
    }
    fn render_lane(&self, down: bool, cx: &Context<Self>) -> AnyElement {
        let lane = self.draft.lane(down);
        let disabled = !lane.is_enabled || (self.draft.id == 0 && self.busy);
        let value = lane.data.first().cloned().unwrap_or_else(|| json!({}));
        let mut content = v_flex()
            .w(surface::css(355.))
            .gap(surface::css(5.))
            .when(disabled, |v| v.opacity(0.5));
        if self.draft.id == 0 {
            let mode = if value["isOff"] == true {
                "off"
            } else if value["isAdvEffect"] == true {
                "advanced"
            } else {
                "quick"
            };
            content = content.child(
                h_flex().gap_1().children(
                    [
                        ("quick", "QUICK_EFFECTS"),
                        ("advanced", "ADVANCED_EFFECTS"),
                        ("off", "CHROMA_LIGHTING_OFF"),
                    ]
                    .map(|(choice, key)| {
                        Button::new(SharedString::from(format!(
                            "automation-chroma-{down}-{choice}"
                        )))
                        .small()
                        .outline()
                        .disabled(disabled)
                        .selected(choice == mode)
                        .label(text(key))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                // yP.handleToggleChange preserves the selected
                                // effect, parameters and both apply flags.
                                let mut value = this
                                    .draft
                                    .lane(down)
                                    .data
                                    .first()
                                    .cloned()
                                    .unwrap_or_else(|| Self::quick(4));
                                value["isAdvEffect"] = json!(choice == "advanced");
                                value["isOff"] = json!(choice == "off");
                                this.choose(down, value, cx);
                                this.sync(window, cx);
                            },
                        ))
                    }),
                ),
            );
            if mode == "quick" {
                let quick_controls = v_flex().child(
                    surface::select(&self.effects[down as usize])
                        .items(Self::effect_choices())
                        .accessibility_label(text("QUICK_EFFECTS"))
                        .disabled(disabled)
                        .w_full(),
                );
                let quick_controls =
                    quick_controls.child(self.render_effect_parameters(down, &value, disabled, cx));
                content = content.child(
                    quick_controls.child(
                        div().pt(surface::css(30.)).child(
                            checkbox::Checkbox::new(SharedString::from(format!(
                                "automation-apply-{down}"
                            )))
                            .checked(value["isApplyToOtherDevices"] == true)
                            .disabled(disabled)
                            .label(text("APPLY_TO_ALL_CHROMA_DEVICES"))
                            .on_click(cx.listener(
                                move |this, on, _, cx| {
                                    let mut value = this
                                        .draft
                                        .lane(down)
                                        .data
                                        .first()
                                        .cloned()
                                        .unwrap_or_else(|| Self::quick(4));
                                    value["isApplyToOtherDevices"] = json!(on);
                                    this.choose(down, value, cx);
                                },
                            )),
                        ),
                    ),
                );
            } else if mode == "off" {
                content = content.child(text("TURN_OFF_CHROMA_LIGHTING")).child(
                    checkbox::Checkbox::new(SharedString::from(format!(
                        "automation-off-all-{down}"
                    )))
                    .checked(value["applyToAllChromaDevices"] == true)
                    .disabled(disabled)
                    .label(text("APPLY_TO_ALL_CHROMA_DEVICES"))
                    .on_click(cx.listener(move |this, on, _, cx| {
                        let mut value = this
                            .draft
                            .lane(down)
                            .data
                            .first()
                            .cloned()
                            .unwrap_or_else(|| Self::quick(4));
                        value["applyToAllChromaDevices"] = json!(on);
                        this.choose(down, value, cx);
                    })),
                );
            } else if self.installed == Some(false) {
                content = content.child(
                    Button::new(SharedString::from(format!("automation-install-{down}")))
                        .outline()
                        .label(text("INSTALL_CHROMA_APP"))
                        .disabled(disabled)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.request("INSTALL_CHROMA_APP", cx)),
                        ),
                );
            } else if self.installed == Some(true) {
                content = content.child(text("NO_CHROMA_STUDIO_PROFILE_TEXT"));
            }
        } else if self.draft.id == 4 && down {
            content = content.child(
                checkbox::Checkbox::new("automation-pause-game")
                    .label(text("PAUSE_GAME_AUTO"))
                    .checked(value["pauseGame"] != false)
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, on, _, cx| {
                        this.choose(down, json!({"pauseGame":on}), cx)
                    })),
            );
        } else {
            if down && matches!(self.draft.id, 1 | 2) {
                content = content.child(self.choice(
                    down,
                    json!({"id":"restore","name":text("RESTORE_PREVIOUS_SETTINGS")}),
                    disabled,
                    cx,
                ));
            }
            let options = self
                .catalogs
                .get(&self.draft.id)
                .cloned()
                .unwrap_or_default();
            if !value.is_null()
                && value.as_object().is_some_and(|v| !v.is_empty())
                && value["id"] != "restore"
                && !options
                    .iter()
                    .any(|o| Self::identity(o) == Self::identity(&value))
            {
                content = content.child(
                    v_flex()
                        .p(surface::css(10.))
                        .border_1()
                        .border_color(Colors::warning())
                        .child(value["name"].as_str().unwrap_or("").to_owned())
                        .child(
                            div()
                                .text_color(Colors::warning())
                                .child(text("SELECTED_DEVICE_DISCONNECTED_OUTPUT_DEVICE")),
                        ),
                );
            }
            for option in options {
                content = content.child(self.choice(down, option, disabled, cx));
            }
            if matches!(self.draft.id, 3 | 4 | 5) {
                let key = if self.draft.id == 5 {
                    "ADD_QUICK_MACRO"
                } else {
                    "ADD"
                };
                content = content.child(
                    Button::new(SharedString::from(format!(
                        "automation-add-candidate-{down}"
                    )))
                    .outline()
                    .disabled(disabled)
                    .label(text(key))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.draft.id == 5 {
                            this.open_macro_picker(down, window, cx);
                        } else {
                            this.request(key, cx);
                        }
                    })),
                );
            }
        }
        content.into_any_element()
    }
    fn identity(value: &Value) -> String {
        ["guid", "id", "path"]
            .into_iter()
            .find_map(|k| value.get(k).map(ToString::to_string))
            .unwrap_or_default()
    }

    /// The live modal gives each service its own card treatment.  The service
    /// payload is intentionally kept opaque here, but these fields are stable
    /// in the current source and let a locally restored draft retain the same
    /// title/detail hierarchy without inventing device data.
    fn option_text(&self, value: &Value) -> (String, Option<String>) {
        let title = value["name"]
            .as_str()
            .or_else(|| value["content"].as_str())
            .or_else(|| value["title"].as_str())
            .unwrap_or("-")
            .to_owned();
        let detail = match self.draft.id {
            3 => value["key"]
                .as_str()
                .or_else(|| value["shortcut"].as_str())
                .or_else(|| value["assignmentValue"].as_str()),
            4 => value["path"]
                .as_str()
                .or_else(|| value["executable"].as_str()),
            5 => value["description"]
                .as_str()
                .or_else(|| value["type"].as_str()),
            _ => value["content"].as_str(),
        };
        (title, detail.map(str::to_owned))
    }
    fn choice(&self, down: bool, value: Value, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let selected = self
            .draft
            .lane(down)
            .data
            .first()
            .is_some_and(|v| Self::identity(v) == Self::identity(&value));
        let (title, detail) = self.option_text(&value);
        let category = self.draft.id;
        let icon_name = if category <= 2 { Some(category) } else { None };
        gpui_kit::base::Button::new(SharedString::from(format!(
            "automation-choice-{down}-{}",
            Self::identity(&value)
        )))
        .accessibility_label(value["name"].as_str().unwrap_or("").to_owned())
        .disabled(disabled)
        .w_full()
        .min_h(surface::css(44.))
        .p(surface::css(10.))
        .rounded(surface::css(5.))
        .bg(Colors::editor())
        .border_1()
        .border_color(if selected {
            Colors::primary()
        } else {
            Colors::editor()
        })
        .text_color(Colors::foreground())
        .hover(|s| s.border_color(Colors::primary().opacity(0.3)))
        .child(
            h_flex()
                .w_full()
                .gap(surface::css(8.))
                .items_center()
                .when_some(icon_name, |row, id| {
                    row.child(icon(id).size(surface::css(20.)))
                })
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(surface::css(2.))
                        .child(div().text_ellipsis().child(title))
                        .when_some(detail, |row, detail| {
                            row.child(
                                div()
                                    .text_size(surface::css(12.))
                                    .text_color(Colors::muted())
                                    .text_ellipsis()
                                    .child(detail),
                            )
                        }),
                )
                .when(selected, |row| {
                    row.child(
                        div()
                            .text_color(Colors::primary())
                            .text_size(surface::css(16.))
                            .child("\u{2713}"),
                    )
                }),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.choose(down, value.clone(), cx)))
        .into_any_element()
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft == self.original || !self.draft.valid() {
            return;
        }
        self.draft.is_enabled = self.draft.pick_up.is_enabled || self.draft.put_down.is_enabled;
        cx.emit(EditorEvent::Save(self.draft.clone()));
        window.close_dialog(cx);
    }
}

fn command(
    id: impl Into<ElementId>,
    label: String,
    primary: bool,
    disabled: bool,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .accessibility_label(label.clone())
        .disabled(disabled)
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(10.))
        .rounded(surface::css(3.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(surface::css(12.))
        .bg(if primary {
            Colors::primary()
        } else {
            Colors::secondary()
        })
        .text_color(if primary {
            Colors::row()
        } else {
            Colors::white()
        })
        .when(disabled, |v| v.opacity(0.4))
        .child(label.to_uppercase())
}
impl Render for AutomationEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = text(if self.editing {
            "EDIT_AUTOMATION"
        } else {
            "ADD_AUTOMATION"
        });
        let empty = spec()
            .empty_messages
            .get(&self.draft.id.to_string())
            .filter(|_| self.catalogs.get(&self.draft.id).is_none_or(Vec::is_empty))
            .map(|key| text(key));
        let headers = h_flex()
            .gap(surface::css(35.))
            .justify_center()
            .mt(surface::css(30.))
            .children([false, true].map(|down| {
                h_flex()
                    .w(surface::css(355.))
                    .gap(surface::css(10.))
                    .child(
                        div().flex_1().child(
                            text(if down {
                                "PUTTING_DOWN_HEAD_SET"
                            } else {
                                "PICKING_UP_HEAD_SET"
                            })
                            .to_uppercase(),
                        ),
                    )
                    .child(
                        surface::SynapseSwitch::new(SharedString::from(format!(
                            "automation-trigger-{down}"
                        )))
                        .checked(self.draft.lane(down).is_enabled)
                        .disabled(self.busy && self.draft.id == 0)
                        .accessibility_label(text(if down {
                            "PUTTING_DOWN_HEAD_SET"
                        } else {
                            "PICKING_UP_HEAD_SET"
                        }))
                        .on_change(cx.listener(
                            move |this, value, window, cx| this.enable(down, *value, window, cx),
                        )),
                    )
            }));
        let body = v_flex()
            .w(surface::css(790.))
            .child(
                v_flex()
                    .p(surface::css(20.))
                    .rounded(surface::css(5.))
                    .bg(Colors::editor())
                    .min_h(surface::css(if self.editing && empty.is_none() {
                        101.
                    } else {
                        125.
                    }))
                    .child(
                        div()
                            .text_color(Colors::muted())
                            .pb(surface::css(10.))
                            .child(text("ACTION")),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .child(if self.editing {
                                h_flex()
                                    .gap(surface::css(5.))
                                    .child(icon(self.draft.id))
                                    .child(text(&spec().actions[self.draft.id as usize].content))
                                    .into_any_element()
                            } else {
                                surface::select(&self.category)
                                    .items(Self::choices(&self.available))
                                    .accessibility_label(text("ACTION"))
                                    .w(surface::css(355.))
                                    .h(surface::css(37.))
                                    .into_any_element()
                            })
                            .child(
                                Button::new("automation-launch-category")
                                    .ghost()
                                    .small()
                                    .label(text(self.launch_key()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(this.launch_key(), cx)
                                    })),
                            ),
                    )
                    .when_some(empty, |v, message| {
                        v.child(
                            div()
                                .mt(surface::css(10.))
                                .text_size(surface::css(12.))
                                .text_color(Colors::warning())
                                .child(message),
                        )
                    }),
            )
            .child(headers)
            .child(
                h_flex()
                    .items_start()
                    .gap(surface::css(34.))
                    .p(surface::css(18.))
                    .child(self.render_lane(false, cx))
                    .child(self.render_lane(true, cx)),
            )
            .when(self.busy && self.draft.id == 0, |v| {
                v.child(
                    div()
                        .text_color(Colors::warning())
                        .child(text("APP_IN_USE")),
                )
            })
            .when_some(self.alert.clone(), |v, message| {
                v.child(
                    div()
                        .px(surface::css(20.))
                        .text_color(Colors::warning())
                        .child(message),
                )
            });
        let footer_bounds = self.delete_footer_bounds.clone();
        v_flex()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::foreground())
            .bg(Colors::panel())
            .child(
                h_flex()
                    .justify_between()
                    .px(surface::css(30.))
                    .py(surface::css(20.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(18.))
                            .child(title),
                    )
                    .child(
                        Button::new("automation-close")
                            .ghost()
                            .small()
                            .label("x")
                            .accessibility_label(text("CLOSE"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
            .child(
                div()
                    .id("automation-editor-scroll")
                    .max_h(
                        (window.viewport_size().height - window.rem_size() * (290. / 16.))
                            .max(px(150.)),
                    )
                    .scrollable_both()
                    .px(surface::css(30.))
                    .child(body),
            )
            .child(
                h_flex()
                    .relative()
                    .justify_center()
                    // `.modal-patch-notes .modal-footer` has greater
                    // specificity than `.automation-footer`.
                    .py(surface::css(10.))
                    .px(surface::css(30.))
                    .border_t_1()
                    .border_color(Colors::delete_cancel_hover())
                    .items_center()
                    .child(self.delete_trigger(window, cx))
                    .child(
                        canvas(
                            move |rect, window, _| {
                                if footer_bounds.replace(rect) != rect {
                                    window.refresh();
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(12.))
                            .child(
                                command("automation-cancel", text("CANCEL"), false, false)
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child(
                                command(
                                    "automation-save",
                                    text("SAVE"),
                                    true,
                                    self.draft == self.original || !self.draft.valid(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                            ),
                    ),
            )
            .children(self.delete_confirmation.clone())
    }
}
