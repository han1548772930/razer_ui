//! UI5235 `$`/`se`: Ctrl/Shift capture, source default Ctrl+Alt, first keyCode
//! lookup for assignment and no generated mapping GUID. Host suppression is
//! owned by the shell worker and restored on blur, unmount and cancellation.
use super::super::audio_seiren_eq::colors::Colors as C;
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::base::{Tooltip as BaseTooltip};

impl AudioProductWorkspace {
    fn emit_seiren_stream_mixer_capture(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        state.capture_guard.store(false, Ordering::Release);
        state.capture_guard = Arc::new(AtomicBool::new(true));
        state.capture_generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        cx.emit(SeirenStreamMixerCaptureChanged {
            owner: state.owner,
            product_id: self.spec.product_id,
            generation: state.capture_generation,
            capturing: state.capture_ready,
            valid: state.capture_guard.clone(),
        });
    }
    fn begin_seiren_stream_mixer_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if state.capturing || state.capture_timer.is_some() {
            return;
        }
        state.capture.focus(window, cx);
        // `$` waits 100ms after the activating mouseup, marks the field active,
        // then waits another 100ms before installing host/input listeners.
        state.capture_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let Ok(started) = this.update_in(cx, |this, window, cx| {
                let Some(state) = &mut this.seiren_stream else {
                    return false;
                };
                if !state.capture.is_focused(window) || !window.is_window_active() {
                    state.capture_timer = None;
                    return false;
                }
                state.capturing = true;
                cx.notify();
                true
            }) else {
                return;
            };
            if !started {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(state) = &mut this.seiren_stream else {
                    return;
                };
                if !state.capturing
                    || !state.capture.is_focused(window)
                    || !window.is_window_active()
                {
                    this.stop_seiren_stream_mixer_capture(cx);
                    return;
                }
                match razer_platform::keyboard_capture::KeyboardCapture::start_on_message_thread() {
                    Ok(capture) => state.raw_capture = Some(capture),
                    Err(error) => {
                        state.error = Some(format!("{error:#}"));
                        state.capturing = false;
                        state.capture_timer = None;
                        cx.notify();
                        return;
                    }
                }
                state.capture_ready = true;
                this.emit_seiren_stream_mixer_capture(cx);
                this.poll_seiren_stream_mixer_raw_capture(window, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(in crate::features) fn stop_seiren_stream_mixer_capture(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        let announce = state.capture_ready;
        state.capturing = false;
        state.capture_ready = false;
        state.capture_timer = None;
        state.raw_capture = None;
        state.raw_capture_task = None;
        if announce {
            self.emit_seiren_stream_mixer_capture(cx);
        }
        cx.notify();
    }
    fn poll_seiren_stream_mixer_raw_capture(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = self
            .seiren_stream
            .as_ref()
            .expect("capture owner")
            .capture_generation;
        let task = cx.spawn_in(window, async move |owner, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive = owner.update_in(cx, |this, window, cx| {
                    if this.seiren_stream.as_ref().is_none_or(|state| {
                        !state.capture_ready || state.capture_generation != generation
                    }) {
                        return false;
                    }
                    this.drain_seiren_stream_mixer_keys(window, cx)
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        });
        self.seiren_stream
            .as_mut()
            .expect("capture owner")
            .raw_capture_task = Some(task);
    }
    fn drain_seiren_stream_mixer_keys(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let events = {
            let Some(state) = &mut self.seiren_stream else {
                return false;
            };
            if !state.capture_ready {
                return false;
            }
            let Some(capture) = &mut state.raw_capture else {
                return false;
            };
            capture.drain_on_message_thread()
        };
        match events {
            Ok(events) => {
                for event in events {
                    self.capture_seiren_stream_mixer_raw(event, window, cx);
                }
                true
            }
            Err(error) => {
                if let Some(state) = &mut self.seiren_stream {
                    state.error = Some(format!("{error:#}"));
                }
                self.stop_seiren_stream_mixer_capture(cx);
                false
            }
        }
    }
    fn capture_seiren_stream_mixer_raw(
        &mut self,
        event: razer_platform::keyboard_capture::KeyboardEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if !state.capture_ready {
            return;
        }
        let keys = &recipe(self.spec.product_id).capture_keys;
        if let Some(captured) = state
            .capture_keys
            .accept(keys, &event, state.capture_hypershift)
        {
            self.store_seiren_stream_mixer_shortcut(
                captured.key,
                true,
                captured.hypershift,
                captured.modifiers,
                window,
                cx,
            );
        }
    }
    fn capture_seiren_stream_mixer_key(
        &mut self,
        _: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .seiren_stream
            .as_ref()
            .is_some_and(|state| state.capture_ready)
        {
            // The raw adapter preserves keys GPUI normalizes or only reports
            // as ModifiersChanged. Prevent captured keys from invoking actions.
            cx.stop_propagation();
        }
    }
    fn capture_seiren_stream_mixer_key_up(
        &mut self,
        _: &KeyUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .seiren_stream
            .as_ref()
            .is_some_and(|state| state.capture_ready)
        {
            cx.stop_propagation();
        }
    }
    fn capture_seiren_stream_mixer_mouse(
        &mut self,
        name: &str,
        input_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // WH_GETMESSAGE has observed earlier keyboard messages, but the 16ms
        // poll may not have consumed them when this mouse message arrives.
        if !self.drain_seiren_stream_mixer_keys(window, cx) {
            return;
        }
        let state = self.seiren_stream.as_ref().expect("capture owner");
        let modifiers = state.capture_keys.modifiers.clone();
        let hypershift = Some(state.capture_hypershift);
        self.store_seiren_stream_mixer_shortcut(
            &json!({"name":name,"inputID":input_id}),
            false,
            hypershift,
            modifiers,
            window,
            cx,
        );
    }
    fn store_seiren_stream_mixer_shortcut(
        &mut self,
        key: &impl serde::Serialize,
        key_input: bool,
        hypershift: Option<bool>,
        modifiers: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if !state.capture_ready {
            return;
        }
        let key = serde_json::to_value(key).expect("source shortcut key fields");
        let guid = selected(&state.data).to_owned();
        if matches!(
            key["inputID"].as_str(),
            Some("KEY_DELETE" | "KEY_BACKSPACE")
        ) {
            state.shortcut_warning = false;
            state.shortcut_warning_value = None;
            self.submit_seiren_stream_mixer(
                "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING",
                json!({"newShortcutMapping":{},"changePresetGUID":guid}),
                window,
                cx,
            );
            return;
        }
        let keys = &recipe(self.spec.product_id).capture_keys;
        let current = &state.data["shortcutMapping"];
        let (mapping, display) =
            shortcut_mapping(current, keys, &key, key_input, hypershift, modifiers);
        // xe passes [currentShortcut] as mappingList. There is no whole preset
        // library conflict check in this current component.
        let duplicate =
            shortcut_conflicts(std::slice::from_ref(current), &current["guid"], &display);
        state.shortcut_warning = duplicate;
        state.shortcut_warning_value = duplicate.then_some(display);
        self.submit_seiren_stream_mixer(
            "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING",
            json!({"newShortcutMapping":if duplicate { json!({}) } else { mapping },"changePresetGUID":guid}),
            window, cx,
        );
    }
    pub(super) fn seiren_stream_mixer_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let value = state
            .shortcut_warning_value
            .as_deref()
            .or_else(|| state.data["shortcutMapping"]["keyValue"].as_str())
            .unwrap_or("")
            .to_owned();
        let warning_border = state.shortcut_warning && !state.capturing;
        let field = BaseButton::new("seiren-stream-shortcut")
            .track_focus(&state.capture)
            .capture_key_down(cx.listener(Self::capture_seiren_stream_mixer_key))
            .capture_key_up(cx.listener(Self::capture_seiren_stream_mixer_key_up))
            .w(surface::css(230.))
            .h(surface::css(27.))
            .px(surface::css(5.))
            .py(surface::css(4.))
            .bg(C::surface())
            .border_1()
            .border_color(if state.capturing {
                C::selected()
            } else if state.shortcut_warning {
                C::warning()
            } else {
                C::shortcut_border()
            })
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(if value.is_empty() {
                C::remainder()
            } else {
                C::text()
            })
            .hover(move |style| {
                style.border_color(if warning_border {
                    C::warning()
                } else {
                    C::selected()
                })
            })
            .accessibility_label(t("PRESET_SHORTCUT"))
            .child(if value.is_empty() && !state.capturing {
                t("SHORTCUT_KEY_INPUT")
            } else {
                value
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.begin_seiren_stream_mixer_capture(window, cx)
                }),
            )
            .on_click(cx.listener(|this, event, window, cx| {
                if matches!(event, ClickEvent::Keyboard(_)) {
                    this.begin_seiren_stream_mixer_capture(window, cx);
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.stop_seiren_stream_mixer_capture(cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, _, window, cx| {
                    this.capture_seiren_stream_mixer_mouse(
                        "Scroll Button",
                        "ScrollButton",
                        window,
                        cx,
                    )
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| {
                    this.capture_seiren_stream_mixer_mouse("Right Click", "RightClick", window, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, window, cx| {
                    this.capture_seiren_stream_mixer_mouse("Button 4", "Button4", window, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| {
                    this.capture_seiren_stream_mixer_mouse("Button 5", "Button5", window, cx)
                }),
            );
        h_flex()
            .items_start()
            .when(state.shortcut_warning, |row| {
                row.child(
                    div()
                        .id("seiren-stream-shortcut-warning")
                        .group("seiren-stream-shortcut-warning")
                        .relative()
                        .w(surface::css(20.))
                        .h(surface::css(20.))
                        .mt(surface::css(10.))
                        .mr(surface::css(10.))
                        .flex_shrink_0()
                        .child(
                            razer_assets::resource_svg()
                                .path(view::asset(self.spec.product_id, "shortcut-warning"))
                                .size_full(),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(surface::css(22.))
                                .right(surface::css(2.))
                                .invisible()
                                .group_hover("seiren-stream-shortcut-warning", |style| {
                                    style.visible()
                                })
                                .child(
                                    BaseTooltip::new("seiren-stream-shortcut-warning-content")
                                        .w(surface::css(300.))
                                        .h(surface::css(49.))
                                        .px(surface::css(10.))
                                        .py(surface::css(8.))
                                        .border_1()
                                        .border_color(C::border())
                                        .bg(C::black())
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(16.))
                                        .text_color(C::text())
                                        .child(view::text(self.spec.product_id, "d8g")),
                                ),
                        ),
                )
            })
            .child(field)
            .into_any_element()
    }
}
fn source_capture_key(keys: &[SourceKey], key_code: u32, location: u8) -> Option<&SourceKey> {
    let flag = if location == 2 { 2 } else { 0 };
    keys.iter()
        .find(|key| key.key_code == key_code.to_string() && key.flag == flag)
}
#[derive(Default)]
pub(super) struct CaptureKeys {
    modifiers: Vec<String>,
    last_key: Option<(u32, u32)>,
}
struct CapturedKey<'a> {
    key: &'a SourceKey,
    modifiers: Vec<String>,
    hypershift: Option<bool>,
}
impl CaptureKeys {
    fn accept<'a>(
        &mut self,
        keys: &'a [SourceKey],
        event: &razer_platform::keyboard_capture::KeyboardEvent,
        hypershift: bool,
    ) -> Option<CapturedKey<'a>> {
        let code = event.key_code();
        let identity = (code, event.physical_code());
        if !event.key_up() && (code == 18 || self.last_key == Some(identity)) {
            return None;
        }
        let key = source_capture_key(keys, code, event.location())?;
        if !event.key_up() {
            // Browser source compares its previous keyCode/code, rather than
            // event.repeat. A first held event after activation is accepted.
            self.last_key = Some(identity);
            if matches!(code, 16 | 17) {
                self.modifiers.push(key.input_id.as_deref()?.into());
                None
            } else {
                Some(CapturedKey {
                    key,
                    modifiers: self.modifiers.clone(),
                    hypershift: Some(hypershift),
                })
            }
        } else {
            let captured = (code == 44).then(|| CapturedKey {
                key,
                modifiers: self.modifiers.clone(),
                // `$` deliberately omits isHyperShift on PrintScreen keyup.
                hypershift: None,
            });
            if code == 44 {
                self.modifiers.clear();
            }
            if matches!(code, 16 | 17) {
                let key_code = code.to_string();
                self.modifiers.retain(|id| {
                    !keys
                        .iter()
                        .any(|key| key.input_id.as_deref() == Some(id) && key.key_code == key_code)
                });
            } else {
                self.last_key = None;
            }
            captured
        }
    }
}
fn shortcut_mapping(
    current: &Value,
    keys: &[SourceKey],
    key: &impl serde::Serialize,
    key_input: bool,
    hypershift: Option<bool>,
    modifiers: Vec<String>,
) -> (Value, String) {
    let key = serde_json::to_value(key).expect("source shortcut key fields");
    let modifiers = if modifiers.is_empty() {
        ["Ctrl", "Alt"]
            .iter()
            .map(|name| {
                keys.iter()
                    .find(|key| key.name == *name)
                    .expect("source default modifier")
                    .input_id
                    .as_deref()
                    .expect("source modifier ID")
                    .to_owned()
            })
            .collect()
    } else {
        modifiers
    };
    let modifier_text = modifiers
        .iter()
        .filter_map(|id| {
            keys.iter()
                .find(|key| key.input_id.as_deref() == Some(id))
                .map(|key| key.name.as_str())
        })
        .map(|name| format!("{name} + "))
        .collect::<String>();
    let display = format!(
        "{}{}{}",
        modifier_text,
        if hypershift == Some(true) {
            "Hypershift + "
        } else {
            ""
        },
        key["name"].as_str().unwrap_or("")
    );
    let mut mapping = if current.is_object() {
        current.clone()
    } else {
        json!({})
    };
    mapping["inputType"] = json!(if key_input { "KeyInput" } else { "MouseInput" });
    let fields = mapping.as_object_mut().expect("mapping object");
    fields.remove(if key_input { "mouseInput" } else { "keyInput" });
    if let Some(hypershift) = hypershift {
        fields.insert("isHyperShift".into(), json!(hypershift));
    } else {
        fields.remove("isHyperShift");
    }
    mapping["inputID"] = if key_input {
        keys.iter()
            .find(|candidate| Some(candidate.key_code.as_str()) == key["keyCode"].as_str())
            .map(|key| json!(key.input_id))
            .unwrap_or(Value::Null)
    } else {
        key["inputID"].clone()
    };
    mapping["inputModifiers"] = json!(modifiers);
    mapping["keyValue"] = json!(display);
    (mapping, display)
}
fn shortcut_conflicts(mappings: &[Value], current_guid: &Value, display: &str) -> bool {
    mappings
        .iter()
        .any(|mapping| mapping["keyValue"] == display && &mapping["guid"] != current_guid)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source_keys() -> &'static [SourceKey] {
        &recipe(1422).capture_keys
    }
    fn event(
        message: u32,
        code: u32,
        scan: usize,
        extended: bool,
        repeated: bool,
    ) -> razer_platform::keyboard_capture::KeyboardEvent {
        razer_platform::keyboard_capture::KeyboardEvent::from_windows_message(
            message,
            code,
            (scan << 16) | (usize::from(extended) << 24) | (usize::from(repeated) << 30),
        )
        .unwrap()
    }
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn stream_mixer_raw_keys_use_browser_sides_and_first_keycode_assignment() {
        let keys = source_keys();
        assert_eq!(
            source_capture_key(keys, 17, 1)
                .unwrap()
                .input_id
                .as_deref()
                .unwrap(),
            "KEY_LEFT_CTRL"
        );
        assert_eq!(
            source_capture_key(keys, 17, 2)
                .unwrap()
                .input_id
                .as_deref()
                .unwrap(),
            "KEY_RIGHT_CTRL"
        );
        assert_eq!(
            source_capture_key(keys, 16, 2)
                .unwrap()
                .input_id
                .as_deref()
                .unwrap(),
            "KEY_RIGHT_SHIFT"
        );
        let num_enter = source_capture_key(keys, 13, 3).unwrap();
        assert_eq!(num_enter.input_id.as_deref().unwrap(), "KEY_ENTER");
        let (mapping, display) =
            shortcut_mapping(&json!({}), keys, num_enter, true, Some(false), vec![]);
        assert_eq!(mapping["inputID"], "KEY_NUMPAD_ENTER");
        assert_eq!(
            mapping["inputModifiers"],
            json!(["KEY_LEFT_CTRL", "KEY_LEFT_ALT"])
        );
        assert_eq!(display, "Ctrl + Alt + Enter");
        assert_eq!(
            source_capture_key(keys, 97, 3)
                .unwrap()
                .input_id
                .as_deref()
                .unwrap(),
            "KEY_NUMPAD_1"
        );
        assert!(source_capture_key(keys, 92, 2).is_none());
    }
    #[test]
    fn stream_mixer_raw_keys_preserve_source_repeat_and_modifier_release() {
        let keys = source_keys();
        let mut capture = CaptureKeys::default();
        assert!(
            capture
                .accept(keys, &event(0x100, 17, 0x1d, false, false), false)
                .is_none()
        );
        assert!(
            capture
                .accept(keys, &event(0x100, 17, 0x1d, false, true), false)
                .is_none()
        );
        assert_eq!(capture.modifiers, vec!["KEY_LEFT_CTRL"]);
        assert!(
            capture
                .accept(keys, &event(0x100, 17, 0x1d, true, false), false)
                .is_none()
        );
        assert_eq!(capture.modifiers, vec!["KEY_LEFT_CTRL", "KEY_RIGHT_CTRL"]);
        capture.accept(keys, &event(0x101, 17, 0x1d, false, true), false);
        assert!(capture.modifiers.is_empty());
        // Modifier release does not reset the preceding keyCode/code pair.
        capture.accept(keys, &event(0x100, 17, 0x1d, true, false), false);
        assert!(capture.modifiers.is_empty());
        assert!(
            capture
                .accept(keys, &event(0x100, 18, 0x38, false, false), false)
                .is_none()
        );
        let primary = capture
            .accept(keys, &event(0x100, 65, 0x1e, false, true), true)
            .unwrap();
        assert_eq!(primary.key.input_id.as_deref().unwrap(), "KEY_A");
        assert_eq!(primary.hypershift, Some(true));
        assert!(
            capture
                .accept(keys, &event(0x100, 65, 0x1e, false, true), true)
                .is_none()
        );
        capture.accept(keys, &event(0x101, 255, 0, false, true), false);
        assert!(
            capture
                .accept(keys, &event(0x100, 65, 0x1e, false, true), false)
                .is_none()
        );
        capture.accept(keys, &event(0x101, 65, 0x1e, false, true), false);
        assert!(
            capture
                .accept(keys, &event(0x100, 65, 0x1e, false, false), false)
                .is_some()
        );
    }
    #[test]
    fn stream_mixer_print_keyup_omits_hypershift_and_clears_recorded_modifiers() {
        let keys = source_keys();
        let mut capture = CaptureKeys::default();
        capture.accept(keys, &event(0x100, 16, 0x36, false, false), true);
        let captured = capture
            .accept(keys, &event(0x101, 44, 0x37, true, true), true)
            .unwrap();
        assert_eq!(captured.modifiers, vec!["KEY_RIGHT_SHIFT"]);
        assert_eq!(captured.hypershift, None);
        let current = json!({"guid":"existing", "mouseInput":"old", "isHyperShift":true});
        let (mapping, display) = shortcut_mapping(
            &current,
            keys,
            captured.key,
            true,
            captured.hypershift,
            captured.modifiers,
        );
        assert_eq!(display, "Right Shift + Print Screen");
        assert_eq!(mapping["inputID"], "KEY_PRINT_SCREEN");
        assert_eq!(mapping["guid"], "existing");
        assert!(mapping.get("mouseInput").is_none());
        assert!(mapping.get("isHyperShift").is_none());
        assert!(capture.modifiers.is_empty());
        assert_eq!(capture.last_key, None);
    }
    #[test]
    fn stream_mixer_mouse_shortcuts_reuse_recorded_modifiers_and_mapping_identity() {
        let keys = source_keys();
        let current = json!({"guid":"original", "keyInput":"old"});
        let (mapping, display) = shortcut_mapping(
            &current,
            keys,
            &json!({"name":"Button 4","inputID":"Button4"}),
            false,
            Some(true),
            vec!["KEY_LEFT_SHIFT".into()],
        );
        assert_eq!(display, "Shift + Hypershift + Button 4");
        assert_eq!(mapping["inputType"], "MouseInput");
        assert_eq!(mapping["inputID"], "Button4");
        assert_eq!(mapping["inputModifiers"], json!(["KEY_LEFT_SHIFT"]));
        assert_eq!(mapping["guid"], "original");
        assert!(mapping.get("keyInput").is_none());
    }
    #[test]
    fn stream_mixer_shortcut_conflicts_follow_the_supplied_current_list() {
        let current = json!({"keyValue":"Ctrl + Alt + A"});
        assert!(!shortcut_conflicts(
            std::slice::from_ref(&current),
            &current["guid"],
            "Ctrl + Alt + A"
        ));
        let same = json!({"guid":"one","keyValue":"Ctrl + Alt + A"});
        assert!(!shortcut_conflicts(
            std::slice::from_ref(&same),
            &json!("one"),
            "Ctrl + Alt + A"
        ));
        assert!(shortcut_conflicts(
            std::slice::from_ref(&same),
            &json!("two"),
            "Ctrl + Alt + A"
        ));
        assert!(!shortcut_conflicts(&[], &json!("two"), "Ctrl + Alt + A"));
    }
}
