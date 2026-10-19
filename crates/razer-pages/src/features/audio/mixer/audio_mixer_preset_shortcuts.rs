//! Current GU/yU capture → ON_CHANGE_PRESET_MAPPING → global registration.
use super::*;
impl AudioProductWorkspace {
    pub fn preset_shortcut_request(&self) -> Option<PresetShortcutRequest> {
        let state = self.effect_presets.as_ref()?;
        let mut registration_errors = Vec::new();
        let bindings = state
            .library
            .presets
            .iter()
            .filter_map(|preset| {
                let mapping = &preset["mapping"];
                let input_id = mapping["inputID"].as_str()?;
                let key = recipe()
                    .shortcut_keys
                    .iter()
                    .find(|key| key.input_id == input_id);
                let Some(key) = key else {
                    // Current MW 19148.v converts a missing 96634 key to NaN.
                    // Keep the persisted source mapping and expose the failure
                    // to diagnostics, without inventing a mouse hook binding.
                    registration_errors.push(format!(
                        "Preset {}: current shortcut key table has no {input_id}",
                        preset["name"]
                    ));
                    return None;
                };
                let mut modifiers = 0;
                let Some(source_modifiers) = mapping["inputModifiers"].as_array() else {
                    registration_errors.push(format!(
                        "Preset {}: shortcut modifiers are not an array",
                        preset["name"]
                    ));
                    return None;
                };
                for modifier in source_modifiers {
                    modifiers |= match modifier.as_str() {
                        Some("KEY_LEFT_ALT") => 256,
                        Some("KEY_LEFT_CTRL") => 512,
                        Some("KEY_LEFT_SHIFT") => 1024,
                        Some("KEY_LEFT_GUI") => 2048,
                        Some("KEY_RIGHT_ALT") => 16,
                        Some("KEY_RIGHT_CTRL") => 32,
                        Some("KEY_RIGHT_SHIFT") => 64,
                        Some("KEY_RIGHT_GUI") => 128,
                        _ => {
                            registration_errors.push(format!(
                                "Preset {}: unknown shortcut modifier {modifier}",
                                preset["name"]
                            ));
                            return None;
                        }
                    };
                }
                Some(PresetShortcutBinding {
                    virtual_key: key.virtual_key,
                    modifiers,
                    preset_guid: preset["guid"].as_str()?.into(),
                })
            })
            .collect();
        Some(PresetShortcutRequest {
            generation: state.shortcut_generation,
            bindings,
            capturing: state.capture_ready,
            registration_errors,
        })
    }
    pub(super) fn emit_preset_shortcuts(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.effect_presets {
            state.shortcut_generation = state.shortcut_generation.wrapping_add(1);
        }
        if let Some(request) = self.preset_shortcut_request() {
            cx.emit(request);
        }
    }
    pub fn preset_shortcut_current(&self, generation: u64) -> bool {
        self.effect_presets
            .as_ref()
            .is_some_and(|state| state.shortcut_generation == generation)
    }
    pub fn finish_preset_shortcuts(
        &mut self,
        generation: u64,
        error: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preset_shortcut_current(generation) {
            return;
        }
        if let Some(error) = &error {
            eprintln!("Audio Mixer preset shortcut: {error}");
        }
        if let Some(state) = &mut self.effect_presets {
            state.shortcut_error = error;
        }
        cx.notify();
    }
    pub fn activate_preset_shortcut(
        &mut self,
        binding: &PresetShortcutBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mixer_io.active {
            return;
        }
        if self
            .preset_shortcut_request()
            .is_some_and(|request| !request.capturing && request.bindings.contains(binding))
        {
            self.select_effect_preset(&binding.preset_guid, window, cx);
        }
    }
    fn begin_preset_shortcut_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.require_effect_preset_source(cx) {
            return;
        }
        if let Some(state) = &mut self.effect_presets {
            if state.capturing {
                return;
            }
            state.capturing = true;
            state.capture.focus(window, cx);
            state.capture_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(state) = &mut this.effect_presets {
                        if !state.capturing {
                            return;
                        }
                        state.capture_ready = true;
                    }
                    this.emit_preset_shortcuts(cx);
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }
    pub(in crate::features::audio_products) fn stop_preset_shortcut_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.effect_presets else {
            return;
        };
        if !state.capturing {
            return;
        }
        state.capturing = false;
        state.capture_ready = false;
        state.capture_timer = None;
        state.capture_hypershift = false;
        self.emit_preset_shortcuts(cx);
        cx.notify();
    }
    pub fn observe_preset_shortcut_mode(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.effect_presets {
            if state.capture_ready {
                state.capture_hypershift = enabled;
            }
        }
        cx.notify();
    }
    fn capture_preset_shortcut(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .effect_presets
            .as_ref()
            .is_some_and(|state| state.capture_ready)
        {
            return;
        }
        if event.is_held {
            cx.stop_propagation();
            return;
        }
        let token = event.keystroke.key.to_uppercase();
        if matches!(token.as_str(), "ALT" | "CTRL" | "CONTROL" | "SHIFT") {
            cx.stop_propagation();
            return;
        }
        let canonical = match token.as_str() {
            "PAGEUP" => "PAGE UP",
            "PAGEDOWN" => "PAGE DOWN",
            "UP" => "UP",
            "DOWN" => "DOWN",
            "LEFT" => "LEFT",
            "RIGHT" => "RIGHT",
            "ESCAPE" => "ESC",
            "SPACE" => "SPACE",
            "CAPSLOCK" => "CAPS LOCK",
            "PRINTSCREEN" => "PRINT SCREEN",
            "SCROLLLOCK" => "SCROLL LOCK",
            "NUMLOCK" => "NUM LOCK",
            _ => token.as_str(),
        };
        let key = recipe()
            .shortcut_keys
            .iter()
            .find(|key| key.name.to_uppercase() == canonical);
        let Some(key) = key else {
            return;
        };
        let input_id = key.input_id.clone();
        let name = key.name.clone();
        cx.stop_propagation();
        self.store_preset_shortcut(&input_id, &name, true, window, cx);
    }
    fn store_preset_shortcut(
        &mut self,
        input_id: &str,
        name: &str,
        key_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .effect_presets
            .as_ref()
            .is_some_and(|state| state.capture_ready)
        {
            return;
        }
        let state = self.effect_presets.as_mut().unwrap();
        let hypershift = state.capture_hypershift && input_id != "KEY_PRINT_SCREEN";
        if matches!(input_id, "KEY_DELETE" | "KEY_BACKSPACE") {
            if let Some(preset) = state.library.active_mut() {
                preset.as_object_mut().unwrap().remove("mapping");
            }
            state.warning = false;
            state.warning_value = None;
        } else {
            // GU captures Ctrl/Shift; Alt is ignored. With neither, yU supplies
            // source default Ctrl+Alt. Left/right identity uses OS observation.
            let held = razer_platform::global_shortcuts::capture_modifier_sides();
            let mut modifiers = Vec::<(&str, &str)>::new();
            for (bit, id, name) in [
                (512, "KEY_LEFT_CTRL", "Ctrl"),
                (32, "KEY_RIGHT_CTRL", "Right Ctrl"),
                (1024, "KEY_LEFT_SHIFT", "Shift"),
                (64, "KEY_RIGHT_SHIFT", "Right Shift"),
            ] {
                if held & bit != 0 {
                    modifiers.push((id, name));
                }
            }
            if modifiers.is_empty() {
                modifiers.extend([("KEY_LEFT_CTRL", "Ctrl"), ("KEY_LEFT_ALT", "Alt")]);
            }
            let display = format!(
                "{}{}{}",
                modifiers
                    .iter()
                    .map(|(_, name)| format!("{name} + "))
                    .collect::<String>(),
                if hypershift { "Hypershift + " } else { "" },
                name
            );
            let duplicate = state.library.presets.iter().any(|preset| {
                preset["guid"].as_str() != Some(&state.library.active_preset)
                    && preset["mapping"]["keyValue"].as_str() == Some(&display)
            });
            state.warning = duplicate;
            state.warning_value = duplicate.then(|| display.clone());
            if let Some(preset) = state.library.active_mut() {
                if duplicate {
                    preset.as_object_mut().unwrap().remove("mapping");
                } else {
                    let name = preset["name"].clone();
                    let guid = preset["guid"].clone();
                    let mapping_guid = preset["mapping"]["guid"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    preset["mapping"] = json!({"guid":mapping_guid,"keyValue":display,"outputType":"interDeviceGroup","isHyperShift":hypershift,
                        "inputType":if key_input { "KeyInput" } else { "MouseInput" },"inputID":input_id,"inputModifiers":modifiers.iter().map(|(id,_)|id).collect::<Vec<_>>(),
                        "interDeviceGroup":{"interPID":1342,"interDeviceType":"presetNavigationGroup","presetNavigationGroup":{
                            "presetNavigationAssignment":"Specific","name":name,"guid":guid}}});
                }
            }
        }
        self.changed_effect_presets(false, window, cx);
    }
    pub(super) fn render_preset_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.effect_presets.as_ref().unwrap();
        let value = state
            .warning_value
            .as_deref()
            .or_else(|| {
                state
                    .library
                    .active()
                    .and_then(|preset| preset["mapping"]["keyValue"].as_str())
            })
            .unwrap_or("");
        let field = div()
            .id("audio-preset-shortcut-capture")
            .track_focus(&state.capture)
            .capture_key_down(cx.listener(|this, event, window, cx| {
                this.capture_preset_shortcut(event, window, cx)
            }))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("printscreen") {
                    this.store_preset_shortcut(
                        "KEY_PRINT_SCREEN",
                        "Print Screen",
                        true,
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            }))
            .w(surface::css(230.))
            .h(surface::css(27.))
            .px(surface::css(5.))
            .py(surface::css(4.))
            .bg(rgb(0x111111))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .border_1()
            .border_color(rgb(if state.capturing { 0x44d62c } else { 0x515151 }))
            .text_color(rgb(if value.is_empty() { 0x707070 } else { 0xcccccc }))
            .child(if value.is_empty() && !state.capturing {
                t("SHORTCUT_KEY_INPUT")
            } else {
                value.into()
            })
            .when(state.warning, |view| view.border_color(rgb(0xfd8611)))
            .hover(|view| view.border_color(rgb(if state.warning { 0xfd8611 } else { 0x44d62c })))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.begin_preset_shortcut_capture(window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.stop_preset_shortcut_capture(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, _, window, cx| {
                    this.store_preset_shortcut("ScrollButton", "Scroll Button", false, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| {
                    this.store_preset_shortcut("RightClick", "Right Click", false, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, window, cx| {
                    this.store_preset_shortcut("Button4", "Button 4", false, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| {
                    this.store_preset_shortcut("Button5", "Button 5", false, window, cx);
                }),
            )
            .into_any_element();
        h_flex()
            .items_start()
            .when(state.warning, |row| {
                row.child(
                    div()
                        .id("audio-preset-shortcut-warning")
                        .group("audio-preset-shortcut-warning")
                        .relative()
                        .size(surface::css(20.))
                        .mr(surface::css(10.))
                        .mt(surface::css(10.))
                        // Byte-identical to current 1342 icon_warning.6c0cd78b.svg.
                        .child(img("synapse/audio-1352-warning.svg").size_full())
                        .child(
                            div()
                                .invisible()
                                .group_hover("audio-preset-shortcut-warning", |style| {
                                    style.visible()
                                })
                                .absolute()
                                .top(surface::css(22.))
                                .right(surface::css(2.))
                                .w(surface::css(300.))
                                .min_h(surface::css(49.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .bg(rgb(0x000000))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .text_size(surface::css(14.))
                                .text_color(rgb(0xcccccc))
                                .child(t("DUPLICATE_SHORTCUT_MESSAGE")),
                        ),
                )
            })
            .child(field)
            .into_any_element()
    }
}
