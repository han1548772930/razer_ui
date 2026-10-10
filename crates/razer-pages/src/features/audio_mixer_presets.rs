//! 1342 Effects preset actions from current kU/od → middleware nl().
//! The library is local storage; only per-effect DSP readbacks confirm hardware.
use super::*;
use gpui_kit::component::{
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
};
#[path = "audio_mixer_preset_shortcuts.rs"]
mod shortcuts;

#[derive(Clone, Deserialize, serde::Serialize)]
pub(super) struct Library {
    pub presets: Vec<Value>,
    pub active_preset: String,
    pub version: u64,
}

#[derive(Deserialize)]
struct Recipe {
    default_preset: Value,
    restore_order: Vec<String>,
    shortcut_keys: Vec<ShortcutKey>,
}
#[derive(Deserialize)]
struct ShortcutKey {
    name: String,
    input_id: String,
    virtual_key: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetShortcutBinding {
    pub virtual_key: u32,
    pub modifiers: u32,
    pub preset_guid: String,
}
#[derive(Clone)]
pub struct PresetShortcutRequest {
    pub generation: u64,
    pub bindings: Vec<PresetShortcutBinding>,
    pub capturing: bool,
}
impl EventEmitter<PresetShortcutRequest> for AudioProductWorkspace {}
fn recipe() -> &'static Recipe {
    static RECIPE: OnceLock<Recipe> = OnceLock::new();
    RECIPE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/data/audio-mixer-presets-current.json"
        ))
        .expect("source-validated AudioMixer preset recipe")
    })
}
impl Default for Library {
    fn default() -> Self {
        let mut preset = recipe().default_preset.clone();
        // getDeviceDefaultLocalData uses getDefaultPreset(), not the UI
        // reducer's provisional Default row.
        preset["guid"] = json!(uuid::Uuid::new_v4().to_string());
        preset["name"] = json!("Preset");
        Self {
            active_preset: preset["guid"].as_str().unwrap().into(),
            presets: vec![preset],
            version: 1,
        }
    }
}
impl Library {
    fn active(&self) -> Option<&Value> {
        self.presets
            .iter()
            .find(|p| p["guid"].as_str() == Some(&self.active_preset))
    }
    fn active_mut(&mut self) -> Option<&mut Value> {
        self.presets
            .iter_mut()
            .find(|p| p["guid"].as_str() == Some(&self.active_preset))
    }
    fn sorted(&self) -> Vec<&Value> {
        let mut presets = self.presets.iter().collect::<Vec<_>>();
        presets.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_uppercase());
        presets
    }
    fn valid(&self) -> bool {
        !self.presets.is_empty()
            && self.active().is_some()
            && self.presets.iter().all(|p| {
                p["guid"].as_str().is_some_and(|id| !id.is_empty())
                    && p["name"]
                        .as_str()
                        .is_some_and(|name| !name.trim().is_empty())
                    && recipe()
                        .restore_order
                        .iter()
                        .all(|field| p[field].is_object())
            })
            && self.presets.iter().enumerate().all(|(index, p)| {
                !self.presets[..index]
                    .iter()
                    .any(|previous| previous["guid"] == p["guid"])
            })
    }
}

pub(super) struct State {
    library: Library,
    choices: Entity<SelectState<Vec<Choice>>>,
    rename: Entity<InputState>,
    renaming: Option<String>,
    generation: u64,
    delete_timer: Option<Task<()>>,
    capture: FocusHandle,
    capturing: bool,
    warning: bool,
    warning_value: Option<String>,
    shortcut_generation: u64,
}

impl AudioProductWorkspace {
    pub(super) fn initialize_effect_presets(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        let choices = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let rename = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let capture = cx.focus_handle();
        self.subscriptions
            .push(cx.on_focus_out(&capture, window, |this, _, _, cx| {
                if let Some(state) = &mut this.effect_presets {
                    if state.capturing {
                        state.capturing = false;
                        this.emit_preset_shortcuts(cx);
                    }
                }
                cx.notify();
            }));
        self.subscriptions.push(
            cx.subscribe_in(&choices, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_effect_preset(id.as_str(), window, cx);
                }
            }),
        );
        self.subscriptions.push(cx.subscribe_in(
            &rename,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let name = input.read(cx).value().trim().to_owned();
                    this.rename_effect_preset(&name, window, cx);
                }
            },
        ));
        self.effect_presets = Some(State {
            library: Library::default(),
            choices,
            rename,
            renaming: None,
            generation: 0,
            delete_timer: None,
            capture,
            capturing: false,
            warning: false,
            warning_value: None,
            shortcut_generation: 0,
        });
        self.refresh_effect_preset_choices(window, cx);
    }

    pub(super) fn effect_preset_snapshot(&self, snapshot: &mut Value) {
        if let Some(state) = &self.effect_presets {
            snapshot["_audioMixerPresets"] = json!(state.library);
        }
    }

    pub(super) fn restore_effect_presets(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = &mut self.effect_presets {
            state.generation = state.generation.wrapping_add(1);
            state.delete_timer = None;
            state.renaming = None;
            state.capturing = false;
            state.warning = false;
            state.warning_value = None;
            state.library = saved
                .and_then(|v| serde_json::from_value(v["_audioMixerPresets"].clone()).ok())
                .filter(Library::valid)
                .unwrap_or_default();
            if let Some(preset) = state.library.active() {
                for field in &recipe().restore_order {
                    self.draft["device"][field] = preset[field].clone();
                }
            }
        }
        self.refresh_effect_preset_choices(window, cx);
        self.emit_preset_shortcuts(cx);
    }

    pub(super) fn submit_restored_effect_preset(&mut self, cx: &mut Context<Self>) {
        if self.effect_presets.is_some() {
            for field in &recipe().restore_order {
                self.request_mixer_write(&format!("/device/{field}/isEnabled"), None, cx);
            }
        }
        self.emit_preset_shortcuts(cx);
    }

    fn refresh_effect_preset_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.effect_presets else {
            return;
        };
        let choices = state
            .library
            .sorted()
            .into_iter()
            .map(|p| Choice::new(p["guid"].as_str().unwrap(), p["name"].as_str().unwrap()))
            .collect();
        let active = state.library.active_preset.clone();
        self.syncing = true;
        state.choices.update(cx, |select, cx| {
            select.set_items(choices, window, cx);
            select.set_selected_value(&active, window, cx);
        });
        self.syncing = false;
    }

    pub(super) fn retain_active_effect(&mut self, path: &str) {
        let Some(field) = recipe()
            .restore_order
            .iter()
            .find(|field| path.starts_with(&format!("/device/{field}/")))
        else {
            return;
        };
        if let Some(state) = &mut self.effect_presets {
            if let Some(preset) = state.library.active_mut() {
                preset[field] = self.draft["device"][field].clone();
                state.library.version = state.library.version.saturating_add(1);
            }
        }
    }

    fn changed_effect_presets(&mut self, apply: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.effect_presets {
            state.library.version = state.library.version.saturating_add(1);
            state.generation = state.generation.wrapping_add(1);
            state.delete_timer = None;
            if apply {
                state.warning = false;
                state.warning_value = None;
            }
        }
        if apply {
            let preset = self
                .effect_presets
                .as_ref()
                .and_then(|state| state.library.active())
                .cloned();
            if let Some(preset) = preset {
                for field in &recipe().restore_order {
                    self.draft["device"][field] = preset[field].clone();
                }
                // Source nl() submits four independent effect tasks in this
                // order. Replaced unsent work must not leak from the old preset.
                self.mixer_echo_timer = None;
                self.mixer_echo_generation = self.mixer_echo_generation.wrapping_add(1);
                self.mixer_write_queue.retain(|(path, _)| {
                    !recipe()
                        .restore_order
                        .iter()
                        .any(|field| path.starts_with(&format!("/device/{field}/")))
                });
                for field in &recipe().restore_order {
                    self.request_mixer_write(&format!("/device/{field}/isEnabled"), None, cx);
                }
            }
        }
        self.refresh_effect_preset_choices(window, cx);
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        self.emit_preset_shortcuts(cx);
        cx.notify();
    }

    fn select_effect_preset(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        if state.library.active_preset == id
            || !state
                .library
                .presets
                .iter()
                .any(|p| p["guid"].as_str() == Some(id))
        {
            return;
        }
        state.library.active_preset = id.into();
        self.changed_effect_presets(true, window, cx);
    }

    fn add_effect_preset(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        let mut preset = if duplicate {
            state
                .library
                .active()
                .cloned()
                .unwrap_or_else(|| recipe().default_preset.clone())
        } else {
            recipe().default_preset.clone()
        };
        let base = if duplicate {
            preset["name"].as_str().unwrap_or("Preset")
        } else {
            "Preset"
        };
        let name = if duplicate {
            let base = base
                .rsplit_once(" (")
                .filter(|(_, suffix)| {
                    suffix.ends_with(')') && suffix[..suffix.len() - 1].parse::<u64>().is_ok()
                })
                .map_or(base, |(base, _)| base.trim());
            let exists = |name: &str| {
                state
                    .library
                    .presets
                    .iter()
                    .any(|p| p["name"].as_str() == Some(name))
            };
            if !exists(base) {
                base.to_owned()
            } else {
                (1..)
                    .map(|i| format!("{base} ({i})"))
                    .find(|name| !exists(name))
                    .unwrap()
            }
        } else {
            (0..=100)
                .map(|i| {
                    if i == 0 {
                        base.into()
                    } else {
                        format!("{base} {i}")
                    }
                })
                .find(|name: &String| {
                    !state
                        .library
                        .presets
                        .iter()
                        .any(|p| p["name"].as_str() == Some(name))
                })
                .unwrap_or_else(|| "Preset 100".into())
        };
        preset["name"] = json!(name);
        preset["guid"] = json!(uuid::Uuid::new_v4().to_string());
        state.library.active_preset = preset["guid"].as_str().unwrap().into();
        state.library.presets.push(preset);
        self.changed_effect_presets(true, window, cx);
    }

    fn begin_effect_preset_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        let name = state
            .library
            .active()
            .map(|p| p["name"].as_str().unwrap_or("").to_owned())
            .unwrap_or_default();
        state.renaming = Some(state.library.active_preset.clone());
        state.rename.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn rename_effect_preset(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        let Some(id) = state.renaming.take() else {
            return;
        };
        if name.is_empty()
            || name.encode_utf16().count() > 32
            || state
                .library
                .presets
                .iter()
                .any(|p| p["name"].as_str() == Some(name))
        {
            cx.notify();
            return;
        }
        if let Some(preset) = state
            .library
            .presets
            .iter_mut()
            .find(|p| p["guid"].as_str() == Some(&id))
        {
            preset["name"] = json!(name);
        }
        self.changed_effect_presets(false, window, cx);
    }

    fn confirm_effect_preset(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.effect_presets else {
            return;
        };
        if !reset && state.library.presets.len() == 1 {
            return;
        }
        let id = state.library.active_preset.clone();
        let generation = state.generation;
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let id = id.clone();
            dialog
                .title(t(if reset {
                    "RESET_PROFILE_TITLE"
                } else {
                    "DELETE_PROFILE_TITLE"
                }))
                .child(t(if reset {
                    "RESET_PRESET_DESC"
                } else {
                    "DELETE_PROFILE_MSG"
                }))
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(!reset)
                        .cancel_text(t("CANCEL"))
                        .ok_text(t(if reset { "RESET" } else { "DELETE" })),
                )
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        let Some(state) = &mut this.effect_presets else {
                            return;
                        };
                        if state.generation != generation || state.library.active_preset != id {
                            return;
                        }
                        if reset {
                            if let Some(preset) = state.library.active_mut() {
                                let name = preset["name"].clone();
                                *preset = recipe().default_preset.clone();
                                preset["guid"] = json!(id);
                                preset["name"] = name;
                                // Source Reset clears mapping input fields first;
                                // middleware removes the mapping when inputID is undefined.
                                preset.as_object_mut().unwrap().remove("mapping");
                            }
                        } else if state.library.presets.len() > 1 {
                            state
                                .library
                                .presets
                                .retain(|p| p["guid"].as_str() != Some(&id));
                            state.library.active_preset =
                                state.library.sorted()[0]["guid"].as_str().unwrap().into();
                        }
                        this.changed_effect_presets(true, window, cx);
                    });
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }

    fn schedule_effect_preset_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        let generation = state.generation;
        state.delete_timer = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = owner.update_in(cx, |this, window, cx| {
                if this
                    .effect_presets
                    .as_ref()
                    .is_some_and(|state| state.generation == generation)
                {
                    this.confirm_effect_preset(false, window, cx);
                }
            });
        }));
    }

    pub(super) fn render_effect_presets(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(state) = &self.effect_presets else {
            return div().into_any_element();
        };
        let owner = cx.entity();
        let delete_disabled = state.library.presets.len() == 1;
        h_flex()
            .w_full()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(surface::css(10.))
                    .items_end()
                    .child(
                        div()
                            .w(surface::css(276.))
                            .text_size(surface::css(14.))
                            .child(t("PRESET").to_uppercase()),
                    )
                    .child(
                        h_flex()
                            .gap_0()
                            .w(surface::css(276.))
                            .child(if state.renaming.is_some() {
                                Input::new(&state.rename).into_any_element()
                            } else {
                                Select::new(&state.choices)
                                    .w(surface::css(250.))
                                    .into_any_element()
                            })
                            .child(
                                Button::new("audio-effects-preset-menu")
                                    .w(surface::css(26.))
                                    .h(surface::css(26.))
                                    .label("⋯")
                                    .dropdown_menu(move |menu, _, _| {
                                        let add = owner.clone();
                                        let rename = owner.clone();
                                        let duplicate = owner.clone();
                                        let reset = owner.clone();
                                        let delete = owner.clone();
                                        menu.item(PopupMenuItem::new(t("ADD")).on_click(
                                            move |_, window, cx| {
                                                add.update(cx, |this, cx| {
                                                    this.add_effect_preset(false, window, cx)
                                                })
                                            },
                                        ))
                                        .item(PopupMenuItem::new(t("RENAME")).on_click(
                                            move |_, window, cx| {
                                                rename.update(cx, |this, cx| {
                                                    this.begin_effect_preset_rename(window, cx)
                                                })
                                            },
                                        ))
                                        .item(PopupMenuItem::new(t("DUPLICATE")).on_click(
                                            move |_, window, cx| {
                                                duplicate.update(cx, |this, cx| {
                                                    this.add_effect_preset(true, window, cx)
                                                })
                                            },
                                        ))
                                        .item(PopupMenuItem::new(t("RESET")).on_click(
                                            move |_, window, cx| {
                                                reset.update(cx, |this, cx| {
                                                    this.confirm_effect_preset(true, window, cx)
                                                })
                                            },
                                        ))
                                        .item(
                                            PopupMenuItem::new(t("DELETE"))
                                                .disabled(delete_disabled)
                                                .on_click(move |_, window, cx| {
                                                    delete.update(cx, |this, cx| {
                                                        this.schedule_effect_preset_delete(
                                                            window, cx,
                                                        )
                                                    })
                                                }),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(surface::css(10.))
                    .pl(surface::css(10.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .child(t("PRESET_SHORTCUT").to_uppercase()),
                    )
                    .child(self.render_preset_shortcut(cx)),
            )
            .into_any_element()
    }
}
