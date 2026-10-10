//! Current GU/yU capture → ON_CHANGE_PRESET_MAPPING → global registration.
use super::*;
impl AudioProductWorkspace {
    pub fn preset_shortcut_request(&self) -> Option<PresetShortcutRequest> {
        let state = self.effect_presets.as_ref()?;
        let bindings = state
            .library
            .presets
            .iter()
            .filter_map(|preset| {
                let mapping = &preset["mapping"];
                let key = recipe()
                    .shortcut_keys
                    .iter()
                    .find(|key| Some(key.input_id.as_str()) == mapping["inputID"].as_str())?;
                let mut modifiers = 0;
                for modifier in mapping["inputModifiers"].as_array()? {
                    modifiers |= match modifier.as_str()? {
                        "KEY_LEFT_ALT" => 256,
                        "KEY_LEFT_CTRL" => 512,
                        "KEY_LEFT_SHIFT" => 1024,
                        "KEY_LEFT_GUI" => 2048,
                        "KEY_RIGHT_ALT" => 16,
                        "KEY_RIGHT_CTRL" => 32,
                        "KEY_RIGHT_SHIFT" => 64,
                        "KEY_RIGHT_GUI" => 128,
                        _ => return None,
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
            capturing: state.capturing,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preset_shortcut_current(generation) {
            return;
        }
        if let Some(error) = error {
            window.push_notification(error, cx);
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
        if let Some(state) = &mut self.effect_presets {
            state.capturing = true;
            state.capture.focus(window, cx);
        }
        self.emit_preset_shortcuts(cx);
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
            .is_some_and(|state| state.capturing)
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
        cx.stop_propagation();
        let state = self.effect_presets.as_mut().unwrap();
        if matches!(key.input_id.as_str(), "KEY_DELETE" | "KEY_BACKSPACE") {
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
                "{}{}",
                modifiers
                    .iter()
                    .map(|(_, name)| format!("{name} + "))
                    .collect::<String>(),
                key.name
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
                    preset["mapping"] = json!({"guid":mapping_guid,"keyValue":display,"outputType":"interDeviceGroup","isHyperShift":false,
                        "inputType":"KeyInput","inputID":key.input_id,"inputModifiers":modifiers.iter().map(|(id,_)|id).collect::<Vec<_>>(),
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
        div()
            .id("audio-preset-shortcut-capture")
            .track_focus(&state.capture)
            .capture_key_down(cx.listener(|this, event, window, cx| {
                this.capture_preset_shortcut(event, window, cx)
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.begin_preset_shortcut_capture(window, cx)),
            )
            .into_any_element()
    }
}
