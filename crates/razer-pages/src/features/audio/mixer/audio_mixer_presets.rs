//! 1342 Effects preset actions from current kU/od → middleware nl().
//! The library is local storage; only per-effect DSP readbacks confirm hardware.
use super::*;
use gpui_kit::component::{
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
    /// Source mappings that have no key in current MW 96634. Never registered as success.
    pub registration_errors: Vec<String>,
}
impl EventEmitter<PresetShortcutRequest> for AudioProductWorkspace {}
fn recipe() -> &'static Recipe {
    static RECIPE: OnceLock<Recipe> = OnceLock::new();
    RECIPE.get_or_init(|| crate::data::audio_mixer_presets_current!())
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
    confirmation: Option<(bool, String, u64)>,
    capture: FocusHandle,
    capturing: bool,
    capture_ready: bool,
    capture_timer: Option<Task<()>>,
    capture_hypershift: bool,
    shortcut_error: Option<String>,
    warning: bool,
    warning_value: Option<String>,
    shortcut_generation: u64,
}

impl AudioProductWorkspace {
    pub(super) fn dismiss_effect_preset_confirmation(&mut self) {
        if let Some(state) = &mut self.effect_presets {
            state.confirmation = None;
            state.delete_timer = None;
        }
    }
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
                this.stop_preset_shortcut_capture(cx);
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
            confirmation: None,
            capture,
            capturing: false,
            capture_ready: false,
            capture_timer: None,
            capture_hypershift: false,
            shortcut_error: None,
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

    pub(super) fn mixer_customization_presets(&self) -> Vec<(String, String)> {
        self.effect_presets
            .as_ref()
            .map(|state| {
                state
                    .library
                    .presets
                    .iter()
                    .filter_map(|preset| {
                        Some((
                            preset["guid"].as_str()?.to_owned(),
                            preset["name"].as_str()?.to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
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
            state.confirmation = None;
            state.renaming = None;
            state.capturing = false;
            state.capture_ready = false;
            state.capture_timer = None;
            state.capture_hypershift = false;
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
        // Current il() iterates the stored profile's own keys. For the local
        // Default persisted profile: fader, mixerBind, outputMixer, four buses,
        // micSetup and micMonitor. Each ol() bus restores volume then all routes.
        // NoiseGate/Compressor restore has no changeKey and ki/wi do not set
        // parameters; do not synthesize a switch action for those branches.
        if self.spec.product_id == 1342 {
            self.request_mixer_write("/profile/brightness", None, cx);
            self.request_mixer_write("/profile/quickEffects", None, cx);
            let channel = if self.draft["device"]["micSetup"]["useMode"] == 0 {
                "xlr"
            } else {
                "mic"
            };
            self.request_mixer_write("/device/fader/useMode", None, cx);
            self.request_mixer_write("/device/mixerBind/bleepButton", None, cx);
            for channel in [
                channel, "chat", "music", "game", "optical", "lineIn", "system",
            ] {
                self.request_mixer_write(
                    &format!("/device/outputMixerReducer/{channel}/value"),
                    None,
                    cx,
                );
            }
            for reducer in [
                "playbackMixReducer",
                "streamMixReducer",
                "lineOutReducer",
                "voiceChatReducer",
            ] {
                self.request_mixer_write(&format!("/device/{reducer}/volume/value"), None, cx);
                for (channel, _) in audio_mixer_channels::CHANNELS {
                    if reducer == "voiceChatReducer" && channel == "chat" {
                        continue;
                    }
                    self.request_mixer_write(
                        &format!("/device/{reducer}/{channel}/isEnabled"),
                        None,
                        cx,
                    );
                }
            }
            self.request_mixer_write("/device/micSetup", None, cx);
            self.request_mixer_write("/device/micMonitor/value", None, cx);
            self.request_mic_eq_write(cx);
        }
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
            state.confirmation = None;
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
                self.mixer_echo_committed = None;
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

    pub(super) fn select_effect_preset(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                .map(|i| if i == 0 { base.into() } else { format!("{base} {i}") })
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

    fn confirm_effect_preset(&mut self, reset: bool, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        if !reset && state.library.presets.len() == 1 {
            return;
        }
        state.confirmation = Some((reset, state.library.active_preset.clone(), state.generation));
        cx.notify();
    }

    fn apply_effect_preset_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        let Some((reset, id, generation)) = state.confirmation.take() else {
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
                // useKU clears mapping input fields before ON_RESET_PRESET.
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
        self.changed_effect_presets(true, window, cx);
    }

    fn render_effect_preset_confirmation(&self, reset: bool, cx: &mut Context<Self>) -> AnyElement {
        use super::audio_mixer_theme::Colors as C;
        use gpui_kit::base::Button as BaseButton;
        // useBU/nR: an anchored source popup, one destructive button and
        // outside-click dismissal. No modal backdrop or extra Cancel action.
        deferred(
            v_flex()
                .id("audio-effects-preset-confirmation")
                .absolute()
                .top(surface::css(42.))
                .right(surface::css(10.))
                .min_w(surface::css(300.))
                .w(surface::css(300.))
                .p(surface::css(20.))
                .items_center()
                .gap(surface::css(10.))
                .bg(C::surface())
                .border_1()
                .border_color(C::danger())
                .rounded(surface::css(3.))
                .text_size(surface::css(14.))
                .text_color(C::text())
                .occlude()
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.effect_presets {
                            state.confirmation = None;
                        }
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .text_center()
                        .text_color(C::danger())
                        .when(reset, |title| title.font_weight(FontWeight::BOLD))
                        .child(
                            t(if reset {
                                "RESET_PROFILE_TITLE"
                            } else {
                                "DELETE_PROFILE_TITLE"
                            })
                            .to_uppercase(),
                        ),
                )
                .child(div().text_center().child(t(if reset {
                    "RESET_PRESET_DESC"
                } else {
                    "DELETE_PROFILE_MSG"
                })))
                .child(
                    BaseButton::new("audio-effects-preset-confirm")
                        .min_w(surface::css(90.))
                        .h(surface::css(27.))
                        .px(surface::css(5.))
                        .bg(C::danger())
                        .text_color(C::surface())
                        .text_size(surface::css(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(t(if reset { "RESET" } else { "DELETE" }))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.apply_effect_preset_confirmation(window, cx)
                        })),
                ),
        )
        .priority(11)
        .into_any_element()
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
                            .relative()
                            .when_some(state.confirmation.as_ref(), |row, (reset, _, _)| {
                                row.child(self.render_effect_preset_confirmation(*reset, cx))
                            })
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
                                        .separator()
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
