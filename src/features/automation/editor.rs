use super::*;
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
    catalogs: BTreeMap<u32, Vec<Value>>,
    installed: Option<bool>,
    busy: bool,
    preview: bool,
    deleting: bool,
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
        let mut this = Self {
            draft: rule.clone(),
            original: rule,
            editing,
            available,
            category: category.clone(),
            effects,
            catalogs,
            installed,
            busy,
            preview,
            deleting: false,
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
                                this.choose(down, Self::quick(id), cx);
                                this.sync(window, cx);
                            }
                        }
                    }
                },
            ));
        }
        this.sync(window, cx);
        this
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
            format!("示例请求：{}", text(key))
        } else {
            format!("{}暂不可用。", text(key))
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
                                this.choose(
                                    down,
                                    match choice {
                                        "off" => {
                                            json!({"isOff":true,"applyToAllChromaDevices":false})
                                        }
                                        "advanced" => json!({"isAdvEffect":true}),
                                        _ => Self::quick(4),
                                    },
                                    cx,
                                );
                                this.sync(window, cx);
                            },
                        ))
                    }),
                ),
            );
            if mode == "quick" {
                content = content.child(
                    surface::select(&self.effects[down as usize])
                        .items(Self::effect_choices())
                        .accessibility_label(text("QUICK_EFFECTS"))
                        .disabled(disabled)
                        .w_full(),
                );
                content = content.child(
                    checkbox::Checkbox::new(SharedString::from(format!("automation-apply-{down}")))
                        .checked(value["isApplyToOtherDevices"] == true)
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
                            value["isApplyToOtherDevices"] = json!(on);
                            this.choose(down, value, cx);
                        })),
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
                        this.choose(down, json!({"isOff":true,"applyToAllChromaDevices":on}), cx);
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
                    .on_click(cx.listener(move |this, _, _, cx| this.request(key, cx))),
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
    fn choice(&self, down: bool, value: Value, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let selected = self
            .draft
            .lane(down)
            .data
            .first()
            .is_some_and(|v| Self::identity(v) == Self::identity(&value));
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
        .child(value["name"].as_str().unwrap_or("-").to_owned())
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
        let delete = self.deleting.then(|| {
            v_flex()
                .p(surface::css(20.))
                .border_1()
                .border_color(Colors::danger())
                .rounded(surface::css(3.))
                .bg(Colors::panel())
                .gap(surface::css(10.))
                .child(
                    div()
                        .text_size(surface::css(16.))
                        .text_color(Colors::danger())
                        .child(text("DELETE_ACTION").to_uppercase()),
                )
                .child(text("DELETE_ACTION_DESC"))
                .child(
                    h_flex()
                        .gap(surface::css(12.))
                        .child(
                            command("automation-cancel-delete", text("CANCEL"), false, false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.deleting = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            command("automation-confirm-delete", text("DELETE"), false, false)
                                .bg(Colors::danger())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    cx.emit(EditorEvent::Delete(this.draft.id));
                                    window.close_dialog(cx);
                                })),
                        ),
                )
        });
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
                            .label("×")
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
            .children(delete)
            .child(
                h_flex()
                    .relative()
                    .justify_center()
                    .py(surface::css(30.))
                    .px(surface::css(10.))
                    .border_t_1()
                    .border_color(Colors::divider())
                    .child(
                        Button::new("automation-delete")
                            .ghost()
                            .small()
                            .absolute()
                            .left(surface::css(31.))
                            .child(
                                img("synapse/automation-icon_delete.svg").size(surface::css(24.)),
                            )
                            .accessibility_label(text("DELETE"))
                            .disabled(!self.editing)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.deleting = true;
                                cx.notify();
                            })),
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
    }
}
