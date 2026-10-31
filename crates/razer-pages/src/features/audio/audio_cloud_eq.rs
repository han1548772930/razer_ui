//! bL/current Cloud EQ. Source arrays, numeric ids, local preview and temporary
//! editing are preserved independently from service observations.
use super::sound::{Product, message, product, tick};
use super::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::theme::AudioEnhancementColors as Colors;
use std::{cell::Cell, rc::Rc};

fn list(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
fn id(value: &Value) -> Value {
    value["cloudEqualizerId"].clone()
}
fn bands(value: &Value) -> Option<Vec<f32>> {
    let values = value.as_array()?;
    if values.len() != 10 {
        return None;
    }
    values
        .iter()
        .map(|v| v.as_f64().map(|v| v as f32))
        .collect()
}
fn band_json(values: &[f32]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| {
                if value.fract() == 0. {
                    json!(*value as i32)
                } else {
                    json!(*value)
                }
            })
            .collect(),
    )
}
fn preset_name(value: &Value) -> String {
    let name = &value["name"];
    name.get(razer_i18n::locale())
        .or_else(|| name.get("en"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
fn library_preset(value: &Value) -> Value {
    json!({"cloudEqualizerId":value["equalizer_id"],"current":value["band_values"],"icon":value["equalizer_icon"],"id":value["equalizer_id"],"isActive":false,"isEnabled":false,"isModified":false,"name":value["equalizer_name"],"position":0,"type":value["equalizer_category"],"bandLabels":value["band_label_ids"],"isAllowSwapping":value["is_allow_swapping"]})
}
#[derive(Clone)]
struct EqDrag {
    preset: Value,
    stored: bool,
}
struct EqDragPreview(String);
impl Render for EqDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p(surface::css(10.))
            .bg(Colors::background())
            .border_1()
            .border_color(Colors::selected())
            .child(self.0.clone())
    }
}
impl AudioProductWorkspace {
    fn cloud(&self) -> &Value {
        &self.draft["sound"]["cloudEQ"]
    }
    fn selected_cloud_preset(&self) -> Option<&Value> {
        self.cloud()["enabledPresets"]
            .as_array()?
            .iter()
            .find(|preset| id(preset) == self.cloud()["selectedPreset"])
    }
    fn current_cloud_bands(&self) -> Option<Vec<f32>> {
        self.sound
            .eq_preview
            .clone()
            .or_else(|| self.sound.eq_band_data.clone())
            .or_else(|| {
                self.selected_cloud_preset()
                    .and_then(|preset| bands(&preset["current"]))
            })
    }
    pub(super) fn request_current_cloud_eq(&self, cx: &mut Context<Self>) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        // Original mount queries Cloud EQ and separately fetches the esports
        // library for its actual edition. This is a request, not an observed list.
        self.emit_sound(vec![json!({"sourceCall":{"method":"fetchEsportsPresets","args":[self.spec.product_id,self.draft["device"].get("editionId").and_then(Value::as_u64).unwrap_or(0)]}}),
            json!({"redux":{"type":row.actions["getCloudEQ"]},"transport":{"type":row.actions["getCloudEQ"],"payload":{}}})],cx);
    }
    pub(super) fn select_cloud_preset(&mut self, preset: Value, cx: &mut Context<Self>) {
        let rows = list(self.cloud(), "enabledPresets");
        let Some(index) = rows.iter().position(|p| id(p) == preset) else {
            return;
        };
        if self.cloud_thx() && rows[index]["isAllowSwapping"] == json!(true) {
            return;
        }
        self.draft["sound"]["cloudEQ"]["selectedPreset"] = preset.clone();
        self.sound.eq_band_data = self
            .sound
            .eq_local_enabled
            .as_ref()
            .and_then(|rows| rows.iter().find(|row| id(row) == preset))
            .and_then(|row| bands(&row["current"]));
        self.sound.eq_preview = None;
        self.sound.eq_disable_reset = self
            .current_cloud_bands()
            .map(|values| band_json(&values) == rows[index]["default"]);
        self.emit_sound_action("setSelectedPreset", preset, cx);
        // Source a declares `timertick` but postMessage reads `timerTick`:
        // serialized transport therefore omits it in the selection branch.
        self.emit_sound(
            vec![json!({"transport":{"type":"ON_SET_CLOUD_EQ","payload":{"index":index}}})],
            cx,
        );
        self.capture_sound_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    pub(super) fn sound_thx_preset_fallback(&mut self, cx: &mut Context<Self>) {
        // bL.componentDidUpdate checks the previous component-local array,
        // then picks the last current non-swappable preset on a THX rising edge.
        let selected = self.cloud()["selectedPreset"].clone();
        let local = self
            .sound
            .eq_local_enabled
            .clone()
            .unwrap_or_else(|| list(self.cloud(), "enabledPresets"));
        if !local
            .iter()
            .any(|row| id(row) == selected && row["isAllowSwapping"] == json!(true))
        {
            return;
        }
        if let Some(next) = list(self.cloud(), "enabledPresets")
            .into_iter()
            .rev()
            .find(|row| row["isAllowSwapping"] != json!(true))
        {
            self.select_cloud_preset(id(&next), cx);
        }
    }
    fn cloud_thx(&self) -> bool {
        self.draft["sound"]["thxSpatialAudio"]["isEnabled"] == json!(true)
    }
    fn submit_cloud_bands(&mut self, values: Vec<f32>, reset: bool, cx: &mut Context<Self>) {
        let selected = self.cloud()["selectedPreset"].clone();
        let mut enabled = self
            .sound
            .eq_local_enabled
            .clone()
            .unwrap_or_else(|| list(self.cloud(), "enabledPresets"));
        let Some(index) = enabled.iter().position(|preset| id(preset) == selected) else {
            // Source resetZeroEQ changes component-local bandData even with
            // no observed presets; the reducer reports "Preset not found"
            // and sends no native message in that branch.
            self.sound.eq_preview = Some(values.clone());
            self.emit_sound_action(
                "setEQ",
                json!({"enabledPresets":enabled,"eqData":{"name":selected,"current":values}}),
                cx,
            );
            cx.notify();
            return;
        };
        let current = band_json(&values);
        if self.sound.eq_local_enabled.is_none() && !reset {
            if let Some(preset) = self.draft["sound"]["cloudEQ"]["enabledPresets"]
                .as_array_mut()
                .and_then(|rows| rows.iter_mut().find(|row| id(row) == selected))
            {
                preset["custom"] = json!(true);
            }
        }
        enabled[index]["current"] = current.clone();
        enabled[index]["custom"] = json!(!reset);
        let payload =
            json!({"enabledPresets":enabled,"eqData":{"name":selected,"current":current}});
        // The actual reducer replaces bandValues only. Component-local
        // enabledPresets/current must not become a fabricated native read.
        self.sound.eq_local_enabled = Some(enabled);
        self.sound.eq_disable_reset = Some(
            reset
                || self
                    .selected_cloud_preset()
                    .is_some_and(|preset| current == preset["default"]),
        );
        self.sound.eq_band_data = Some(values);
        self.draft["sound"]["cloudEQ"]["bandValues"] = current.clone();
        self.sound.eq_preview = None;
        self.emit_sound_action("setEQ", payload, cx);
        let mut native_payload =
            json!({"index":index,"bandValues":current,"cloudEqualizerId":selected});
        if let Some(custom) = self
            .selected_cloud_preset()
            .and_then(|preset| preset.get("custom"))
        {
            native_payload["custom"] = custom.clone();
        }
        self.emit_sound(vec![json!({"transport":{"type":"ON_SET_CLOUD_EQ","timerTick":tick(),"payload":native_payload}})],cx);
        self.capture_sound_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn finish_cloud_drag(&mut self, cx: &mut Context<Self>) {
        if self.sound.eq_drag.take().is_none() {
            return;
        }
        if let Some(values) = self.sound.eq_preview.take() {
            self.submit_cloud_bands(values, false, cx);
        }
    }
    fn preview_cloud_point(&mut self, index: usize, value: f32, cx: &mut Context<Self>) {
        let Some(mut values) = self.current_cloud_bands() else {
            return;
        };
        if index >= values.len() {
            return;
        }
        values[index] = value.clamp(-6., 6.).round();
        let selected = self.cloud()["selectedPreset"].clone();
        let mut enabled = self
            .sound
            .eq_local_enabled
            .clone()
            .unwrap_or_else(|| list(self.cloud(), "enabledPresets"));
        if let Some(preset) = enabled.iter_mut().find(|preset| id(preset) == selected) {
            preset["current"] = band_json(&values);
            preset["custom"] = json!(true);
            if self.sound.eq_local_enabled.is_none() {
                if let Some(preset) = self.draft["sound"]["cloudEQ"]["enabledPresets"]
                    .as_array_mut()
                    .and_then(|rows| rows.iter_mut().find(|row| id(row) == selected))
                {
                    preset["custom"] = json!(true);
                }
            }
            self.sound.eq_local_enabled = Some(enabled);
        }
        self.sound.eq_disable_reset = Some(
            self.selected_cloud_preset()
                .is_some_and(|preset| band_json(&values) == preset["default"]),
        );
        self.sound.eq_preview = Some(values);
        cx.notify();
    }
    fn open_cloud_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.emit_sound_action("resetTempPresets", Value::Null, cx);
        let cloud = &mut self.draft["sound"]["cloudEQ"];
        cloud["tempEnabledPresets"] = cloud["enabledPresets"].clone();
        cloud["tempDisabledPresets"] = cloud["disabledPresets"].clone();
        cloud["tempActivePreset"] = cloud["selectedPreset"].clone();
        self.sound.edit_eq_shown = true;
        self.sound.edit_eq_changed = false;
        self.sound.eq_remove = None;
        self.sound.eq_reset = false;
        if self.sound.eq_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx));
            self.subscriptions
                .push(cx.subscribe(&input, |_, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }));
            self.sound.eq_search = Some(input);
        }
        cx.notify();
    }
    fn publish_temp_cloud(
        &mut self,
        mut enabled: Vec<Value>,
        mut disabled: Vec<Value>,
        active: Value,
        cx: &mut Context<Self>,
    ) {
        for preset in enabled.iter_mut().chain(disabled.iter_mut()) {
            preset["currentActive"] = json!(id(preset) == active);
        }
        let cloud = &mut self.draft["sound"]["cloudEQ"];
        cloud["tempEnabledPresets"] = json!(enabled);
        cloud["tempDisabledPresets"] = json!(disabled);
        cloud["tempActivePreset"] = active.clone();
        self.sound.edit_eq_changed = true;
        self.emit_sound_action(
            "changeTempPresets",
            json!({"enabled":enabled,"disabled":disabled,"tempActivePreset":active}),
            cx,
        );
        cx.notify();
    }
    fn next_cloud_active(&self, preset: &Value, remaining: &[Value]) -> Value {
        let old = list(self.cloud(), "tempEnabledPresets");
        let position = old.iter().position(|p| id(p) == id(preset)).unwrap_or(0);
        if self.cloud_thx() {
            let candidates = remaining
                .iter()
                .filter(|p| p["isAllowSwapping"] != json!(true))
                .collect::<Vec<_>>();
            return candidates
                .iter()
                .rev()
                .find(|p| {
                    old.iter()
                        .position(|x| id(x) == id(p))
                        .is_some_and(|i| i < position)
                })
                .or_else(|| candidates.first())
                .map(|p| id(p))
                .unwrap_or(Value::Null);
        }
        old.get(position.wrapping_sub(1))
            .filter(|p| remaining.iter().any(|x| id(x) == id(p)))
            .or_else(|| {
                remaining.iter().find(|p| {
                    old.iter()
                        .position(|x| id(x) == id(p))
                        .is_some_and(|i| i > position)
                })
            })
            .or_else(|| remaining.first())
            .map(id)
            .unwrap_or(json!(0))
    }
    fn toggle_cloud_storage(&mut self, preset: Value, stored: bool, cx: &mut Context<Self>) {
        let mut enabled = list(self.cloud(), "tempEnabledPresets");
        let mut disabled = list(self.cloud(), "tempDisabledPresets");
        let mut active = self.cloud()["tempActivePreset"].clone();
        if stored {
            if preset["isAllowSwapping"] != json!(true)
                && enabled
                    .iter()
                    .filter(|p| p["isAllowSwapping"] != json!(true))
                    .count()
                    <= 1
            {
                return;
            }
            if !list(self.cloud(), "availableEsportsPresets")
                .iter()
                .any(|p| p["equalizer_id"] == id(&preset))
            {
                self.sound.eq_remove = Some(preset);
                cx.notify();
                return;
            }
            enabled.retain(|p| id(p) != id(&preset));
            disabled.insert(0, preset.clone());
            if active == id(&preset) {
                active = self.next_cloud_active(&preset, &enabled);
            }
        } else {
            if enabled.len() >= 9 {
                return;
            }
            disabled.retain(|p| id(p) != id(&preset));
            enabled.push(preset.clone());
            if self.cloud()["selectedPreset"] == id(&preset)
                && !(self.cloud_thx() && preset["isAllowSwapping"] == json!(true))
            {
                active = id(&preset);
            }
        }
        self.publish_temp_cloud(enabled, disabled, active, cx);
    }
    fn reorder_cloud_storage(
        &mut self,
        drag: &EqDrag,
        target: Value,
        after: bool,
        cx: &mut Context<Self>,
    ) {
        let mut enabled = list(self.cloud(), "tempEnabledPresets");
        let mut disabled = list(self.cloud(), "tempDisabledPresets");
        if !drag.stored && enabled.len() >= 9 {
            return;
        }
        if id(&drag.preset) == target {
            return;
        }
        enabled.retain(|p| id(p) != id(&drag.preset));
        disabled.retain(|p| id(p) != id(&drag.preset));
        let Some(index) = enabled.iter().position(|p| id(p) == target) else {
            return;
        };
        enabled.insert(index + usize::from(after), drag.preset.clone());
        let active = self.cloud()["tempActivePreset"].clone();
        self.publish_temp_cloud(enabled, disabled, active, cx);
    }
    fn save_cloud_editor(&mut self, cx: &mut Context<Self>) {
        if !self.sound.edit_eq_changed {
            return;
        }
        let enabled = list(self.cloud(), "tempEnabledPresets");
        let disabled = list(self.cloud(), "tempDisabledPresets");
        let selected = self.cloud()["tempActivePreset"].clone();
        let mut voice_prompt = 4;
        let mut transport = Vec::new();
        for (index, (preset, stored)) in enabled
            .iter()
            .map(|p| (p, true))
            .chain(disabled.iter().map(|p| (p, false)))
            .enumerate()
            .take(9)
        {
            let voice = if !stored {
                json!(-1)
            } else if preset["isAllowSwapping"] == json!(true) {
                let value = voice_prompt;
                voice_prompt += 1;
                json!(value)
            } else {
                id(preset)
            };
            let mut row = json!({"index":index,"enableState":stored,"currentActive":id(preset)==selected,"cloudEqualizerId":id(preset),"custom":preset["custom"].as_bool().unwrap_or(false),"voicePromptIndex":voice});
            if let Some(current) = preset.get("current") {
                row["bandValues"] = current.clone();
            }
            transport.push(row);
        }
        let cloud = &mut self.draft["sound"]["cloudEQ"];
        cloud["enabledPresets"] = json!(enabled);
        cloud["disabledPresets"] = json!(disabled);
        cloud["selectedPreset"] = selected;
        self.emit_sound_action("savePresets", Value::Null, cx);
        // Original o.timerTick is undefined (object key is `timertick`).
        self.emit_sound(
            vec![json!({"transport":{"type":"ON_REORDER_CLOUD_EQ","payload":transport}})],
            cx,
        );
        self.sound.edit_eq_shown = false;
        self.sound.edit_eq_changed = false;
        self.sound.eq_preview = None;
        self.sound.eq_local_enabled = None;
        self.sound.eq_band_data = None;
        self.sound.eq_disable_reset = None;
        self.capture_sound_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn reset_cloud_oobe(&mut self, cx: &mut Context<Self>) {
        let mut available = list(self.cloud(), "availableEsportsPresets")
            .into_iter()
            .filter(|p| p["is_oobe"] == json!(true))
            .take(9)
            .collect::<Vec<_>>();
        let mut disabled = available
            .iter()
            .filter(|p| p["is_oobe_enabled"] != json!(true))
            .cloned()
            .collect::<Vec<_>>();
        disabled.sort_by_key(|p| p["equalizer_id"].as_i64().unwrap_or(0));
        available.retain(|p| p["is_oobe_enabled"] == json!(true));
        let active = available
            .iter()
            .find(|p| p["is_default"] == json!(true))
            .map(|p| p["equalizer_id"].clone())
            .unwrap_or(json!(0));
        self.emit_sound_action("resetOobePresets", Value::Null, cx);
        self.publish_temp_cloud(
            available.iter().map(library_preset).collect(),
            disabled.iter().map(library_preset).collect(),
            active,
            cx,
        );
        self.sound.eq_reset = false;
        self.save_cloud_editor(cx);
    }
    fn cloud_band_labels(&self, cx: &Context<Self>) -> AnyElement {
        let labels = self
            .selected_cloud_preset()
            .and_then(|preset| preset["bandLabels"].as_array())
            .cloned()
            .unwrap_or_default();
        h_flex()
            .mt(surface::css(10.))
            .ml(surface::css(10.))
            .w(surface::css(1080.))
            .gap(surface::css(2.))
            .children(labels.iter().enumerate().map(|(index, subset)| {
                let name = subset["label_name"]
                    .get(razer_i18n::locale())
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let count = subset["band_index"].as_array().map_or(0, Vec::len);
                let truncated = count == 1 && name.encode_utf16().count() > 12;
                let text = if subset["label_id"] == json!(-1) {
                    String::new()
                } else if truncated {
                    format!(
                        "{}...",
                        String::from_utf16_lossy(&name.encode_utf16().take(8).collect::<Vec<_>>())
                    )
                } else {
                    name.to_string()
                };
                let highlighted = self.sound.eq_hover.is_some_and(|point| {
                    subset["band_index"]
                        .as_array()
                        .is_some_and(|indices| indices.contains(&json!(point)))
                });
                let hover_subset = subset.clone();
                let label = div()
                    .id(SharedString::from(format!(
                        "current-cloud-band-label-{index}"
                    )))
                    .w(surface::css(108. * count as f32))
                    .p(surface::css(5.))
                    .text_size(surface::css(12.))
                    .text_center()
                    .bg(if highlighted { rgb(0x333333) } else { rgb(0x222222) })
                    .text_color(if highlighted {
                        Colors::selected()
                    } else {
                        Hsla::from(rgb(0xcccccc))
                    })
                    .child(text.to_uppercase())
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if this.sound.eq_drag.is_none()
                            && this.sound.eq_hover_range.as_ref() != Some(&hover_subset)
                        {
                            this.sound.eq_hover_range = Some(hover_subset.clone());
                            this.sound.eq_hover = None;
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered && this.sound.eq_drag.is_none() {
                            this.sound.eq_hover_range = None;
                            cx.notify();
                        }
                    }));
                if truncated && subset["label_id"] != json!(-1) {
                    razer_widgets::source_tooltip::SourceTooltip::new(
                        SharedString::from(format!("current-cloud-band-label-tip-{index}")),
                        name.to_string(),
                        300.,
                    )
                    .trigger(move |_, _, _| label.into_any_element())
                    .into_any_element()
                } else {
                    label.into_any_element()
                }
            }))
            .into_any_element()
    }
    fn cloud_chart(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let bands = self.current_cloud_bands();
        let paint_bands = bands.clone();
        let hover = self.sound.eq_hover;
        let dragging = self.sound.eq_drag;
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let paint_bounds = bounds.clone();
        let down_bounds = bounds.clone();
        let move_bounds = bounds;
        let chart = div()
            .id("current-cloud-eq-chart")
            .relative()
            .w(surface::css(1030.))
            .h(surface::css(300.))
            .child(
                canvas(
                    move |rect, _, _| paint_bounds.set(rect),
                    move |rect, _, window, _| {
                        let scale = f32::from(rect.size.width) / 1030.;
                        let pt = |x: f32, y: f32| {
                            point(rect.left() + px(x * scale), rect.top() + px(y * scale))
                        };
                        if let Some(values) = &paint_bands {
                            let mut path = PathBuilder::stroke(px(2. * scale));
                            for (index, value) in values.iter().enumerate() {
                                let point =
                                    pt(8. + 109.55 * index as f32, 8. + 276. * (6. - *value) / 12.);
                                if index == 0 {
                                    path.move_to(point);
                                } else {
                                    path.line_to(point);
                                }
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, Colors::selected());
                            }
                            for (index, value) in values.iter().enumerate() {
                                let center =
                                    pt(8. + 109.55 * index as f32, 8. + 276. * (6. - *value) / 12.);
                                let color = if dragging == Some(index) {
                                    rgb(0x383838)
                                } else if hover == Some(index) {
                                    rgb(0x5d5d5d)
                                } else {
                                    rgb(0x44d62c)
                                };
                                window.paint_quad(quad(
                                    Bounds::new(
                                        center - point(px(7. * scale), px(7. * scale)),
                                        size(px(14. * scale), px(14. * scale)),
                                    ),
                                    px(7. * scale),
                                    color,
                                    px(0.),
                                    color,
                                    BorderStyle::Solid,
                                ));
                            }
                        }
                    },
                )
                .size_full(),
            )
            .when(bands.is_some(), |body| {
                body.children(row.configs.equalizer.frequencies.iter().enumerate().map(
                    |(index, label)| {
                        let range_hovered = self.sound.eq_hover.is_none()
                            && self.sound.eq_hover_range.as_ref().is_some_and(|range| {
                                range["band_index"]
                                    .as_array()
                                    .is_some_and(|indices| indices.contains(&json!(index)))
                            });
                        div()
                            .absolute()
                            .left(surface::css(109.55 * index as f32))
                            .top(surface::css(300.))
                            .w(surface::css(16.))
                            .text_size(surface::css(12.))
                            .text_center()
                            .text_color(if self.sound.eq_hover == Some(index) || range_hovered {
                                Colors::selected()
                            } else {
                                Hsla::from(rgb(0xcccccc))
                            })
                            .child(label.clone())
                    },
                ))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    let rect = down_bounds.get();
                    if rect.size.width <= px(0.) {
                        return;
                    }
                    let scale = f32::from(rect.size.width) / 1030.;
                    let x = f32::from(event.position.x - rect.left()) / scale;
                    let index = (x / 109.55).round().clamp(0., 9.) as usize;
                    if (x - (8. + 109.55 * index as f32)).abs() > 8.5
                        || this.current_cloud_bands().is_none()
                    {
                        return;
                    }
                    this.sound.eq_drag = Some(index);
                    let y = f32::from(event.position.y - rect.top()) / scale;
                    this.preview_cloud_point(index, 6. - ((y - 8.) / (276. / 12.)).round(), cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                let rect = move_bounds.get();
                if rect.size.width <= px(0.) {
                    return;
                }
                let scale = f32::from(rect.size.width) / 1030.;
                let x = f32::from(event.position.x - rect.left()) / scale;
                let index = (x / 109.55).round().clamp(0., 9.) as usize;
                let hovered = ((x - (8. + 109.55 * index as f32)).abs() <= 8.5).then_some(index);
                this.sound.eq_hover = hovered;
                if let Some(index) = this.sound.eq_drag {
                    let y = f32::from(event.position.y - rect.top()) / scale;
                    this.preview_cloud_point(index, 6. - ((y - 8.) / (276. / 12.)).round(), cx);
                } else {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_cloud_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.sound.eq_hover = None;
                    this.finish_cloud_drag(cx);
                }),
            );
        h_flex()
            .items_start()
            .child(chart)
            .child(
                h_flex()
                    .w(surface::css(100.))
                    .items_start()
                    .child(v_flex().h(surface::css(300.)).justify_between().children(
                        row.configs.equalizer.y_axis_title.iter().map(|label| {
                            div().text_color(Colors::secondary()).child(label.clone())
                        }),
                    ))
                    .child(
                        div()
                            .w(surface::css(20.))
                            .h(surface::css(297.))
                            .mt(surface::css(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .id("current-reset-zero-eq")
                                    .size(surface::css(20.))
                                    .opacity(0.85)
                                    .cursor_pointer()
                                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                        this.sound.eq_zero_hovered = *hovered;
                                        cx.notify();
                                    }))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.sound.eq_zero_pressed = true;
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.sound.eq_zero_pressed = false;
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_up_out(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.sound.eq_zero_pressed = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        img(row
                                            .media
                                            .css
                                            .get(if self.sound.eq_zero_pressed {
                                                "eq_reset_active"
                                            } else if self.sound.eq_zero_hovered {
                                                "eq_reset_hover"
                                            } else {
                                                "eq_reset"
                                            })
                                            .cloned()
                                            .unwrap_or_default())
                                        .size_full(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.submit_cloud_bands(vec![0.; 10], false, cx);
                                        this.sound.eq_disable_reset = Some(false);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn cloud_preset_card(
        &self,
        preset: &Value,
        index: usize,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        let preset_id = id(preset);
        let ordinal = list(self.cloud(), "enabledPresets")
            .into_iter()
            .filter(|item| item["isAllowSwapping"] == json!(true))
            .position(|item| id(&item) == preset_id)
            .map(|index| index + 1);
        let disabled = self.cloud_thx() && preset["isAllowSwapping"] == json!(true);
        let selected = preset_id == self.cloud()["selectedPreset"];
        let icon = if preset["isAllowSwapping"] == json!(true) {
            Some(row.media.sword_added_icon.as_str())
        } else {
            row.media
                .eq_icons
                .get(&preset_id.to_string())
                .map(String::as_str)
        };
        let card = razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!(
            "current-eq-preset-{preset_id}"
        )))
        .disabled(disabled)
        .w(surface::css(220.))
        .h(surface::css(106.))
        .p_0()
        .rounded(surface::css(3.))
        .border_1()
        .border_color(if selected {
            Colors::selected()
        } else {
            Colors::border()
        })
        .bg(Colors::background())
        .when(disabled, |card| card.opacity(0.3))
        .child(
            v_flex()
                .w_full()
                .child(
                    div()
                        .h(surface::css(73.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(icon, |body, icon| {
                            body.child(
                                img(icon.to_string())
                                    .max_w(surface::css(180.))
                                    .max_h(surface::css(60.))
                                    .object_fit(ObjectFit::Contain),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .h(surface::css(31.))
                        .w_full()
                        .px(surface::css(10.))
                        .justify_between()
                        .child(if preset["isAllowSwapping"] == json!(true) {
                            message(row, "ESPORTS_EQ")
                        } else {
                            preset_name(preset)
                        })
                        .when_some(ordinal, |body, number| body.child(number.to_string())),
                ),
        )
        .on_click(
            cx.listener(move |this, _, _, cx| this.select_cloud_preset(preset_id.clone(), cx)),
        );
        if disabled {
            razer_widgets::source_tooltip::SourceTooltip::new(
                SharedString::from(format!("current-esports-disabled-{index}")),
                message(row, "ESPORTS_EQ_TOOLTIP"),
                300.,
            )
            .trigger(move |_, _, _| card.into_any_element())
            .into_any_element()
        } else {
            card.into_any_element()
        }
    }
    pub(super) fn current_cloud_eq(
        &self,
        row: &Product,
        _: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = list(self.cloud(), "enabledPresets");
        let reset_disabled = self.sound.eq_disable_reset.unwrap_or_else(|| {
            self.selected_cloud_preset().is_none_or(|preset| {
                self.current_cloud_bands()
                    .map(|values| band_json(&values) == preset["default"])
                    .unwrap_or(true)
            })
        });
        surface::panel_with_help(
            message(row, "AUDIO_EQUALIZER"),
            surface::help_control("current-cloud-eq-tip", message(row, "EQUALIZER_TOOLTIP_2")),
            cx,
        )
        .mt(surface::css(20.))
        .mb(surface::css(10.))
        .child(
            div()
                .text_color(Colors::secondary())
                .child(message(row, "AUDIO_FUNCTION_EQ_SAVED_TO_HEADSET_DES")),
        )
        .child(
            razer_widgets::keyboard_navigation::Button::new("current-edit-eq-list")
                .self_end()
                .child(message(row, "EDIT_EQ_LIST"))
                .on_click(cx.listener(|this, _, window, cx| this.open_cloud_editor(window, cx))),
        )
        .child(
            h_flex()
                .gap(surface::css(10.))
                .children(
                    enabled
                        .iter()
                        .enumerate()
                        .map(|(index, preset)| self.cloud_preset_card(preset, index, row, cx)),
                )
                .when(enabled.len() < 5, |body| {
                    body.child(
                        razer_widgets::keyboard_navigation::Button::new("current-add-eq-preset")
                            .w(surface::css(220.))
                            .h(surface::css(106.))
                            .border_1()
                            .border_color(Colors::border())
                            .child(
                                v_flex()
                                    .items_center()
                                    .child(div().text_size(surface::css(28.)).child("+"))
                                    .child(message(row, "ADD_MORE_EQ_PRESET")),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_cloud_editor(window, cx)
                            })),
                    )
                }),
        )
        .child(self.cloud_chart(row, cx))
        .child(self.cloud_band_labels(cx))
        .child(
            razer_widgets::keyboard_navigation::Button::new("current-reset-current-eq")
                .disabled(reset_disabled)
                .self_center()
                .mt(surface::css(20.))
                .px(surface::css(15.))
                .py(surface::css(6.))
                .rounded(surface::css(3.))
                .bg(rgb(0x707070))
                .text_color(rgb(0xffffff))
                .text_size(surface::css(12.))
                .hover(|style| style.opacity(0.8))
                .child(
                    img(row.media.reset_link.clone())
                        .size(surface::css(20.))
                        .mr(surface::css(5.)),
                )
                .child(message(row, "RESET_CURRENT_EQ").to_uppercase())
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(values) = this
                        .selected_cloud_preset()
                        .and_then(|preset| bands(&preset["default"]))
                    {
                        this.submit_cloud_bands(values, true, cx);
                    }
                })),
        )
        .into_any_element()
    }
    fn cloud_editor_card(&self, preset: &Value, stored: bool, cx: &Context<Self>) -> AnyElement {
        let last_standard = stored
            && preset["isAllowSwapping"] != json!(true)
            && list(self.cloud(), "tempEnabledPresets")
                .iter()
                .filter(|p| p["isAllowSwapping"] != json!(true))
                .count()
                == 1;
        let full = !stored && list(self.cloud(), "tempEnabledPresets").len() >= 9;
        let source = preset.clone();
        let ordinal = list(self.cloud(), "tempEnabledPresets")
            .into_iter()
            .filter(|item| item["isAllowSwapping"] == json!(true))
            .position(|item| id(&item) == id(preset))
            .map(|index| index + 1);
        let target = id(preset);
        let card_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let set_bounds = card_bounds.clone();
        div()
            .id(SharedString::from(format!(
                "current-edit-eq-{stored}-{target}"
            )))
            .relative()
            .w(surface::css(150.))
            .h(surface::css(115.))
            .border_1()
            .border_color(Colors::border())
            .rounded(surface::css(3.))
            .bg(Colors::background())
            .when(last_standard || full, |body| body.opacity(0.3))
            .child(
                canvas(move |bounds, _, _| set_bounds.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                v_flex()
                    .p(surface::css(10.))
                    .child(preset_name(preset))
                    .when_some(ordinal, |body, number| body.child(number.to_string()))
                    .child(
                        razer_widgets::keyboard_navigation::Button::new(SharedString::from(
                            format!("current-eq-store-{stored}-{target}"),
                        ))
                        .disabled(last_standard || full)
                        .child(if stored { "−" } else { "+" })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_cloud_storage(source.clone(), stored, cx)
                        })),
                    ),
            )
            .when(!last_standard && !full, |body| {
                let drag = EqDrag {
                    preset: preset.clone(),
                    stored,
                };
                body.on_drag(drag, |drag, _, _, cx| {
                    cx.new(|_| EqDragPreview(preset_name(&drag.preset)))
                })
            })
            .when(stored, |body| {
                body.on_drop(cx.listener(move |this, drag: &EqDrag, window, cx| {
                    let after = window.mouse_position().x > card_bounds.get().center().x;
                    this.reorder_cloud_storage(drag, target.clone(), after, cx);
                }))
            })
            .into_any_element()
    }
    pub(super) fn current_cloud_eq_editor(
        &self,
        row: &Product,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = list(self.cloud(), "tempEnabledPresets");
        let disabled = list(self.cloud(), "tempDisabledPresets");
        let available = list(self.cloud(), "availableEsportsPresets");
        let mut library = disabled.clone();
        for preset in &available {
            if !enabled
                .iter()
                .chain(disabled.iter())
                .any(|p| id(p) == preset["equalizer_id"])
            {
                library.push(library_preset(preset));
            }
        }
        library.sort_by_key(|p| {
            available
                .iter()
                .position(|v| v["equalizer_id"] == id(p))
                .unwrap_or(usize::MAX)
        });
        let search = self
            .sound
            .eq_search
            .as_ref()
            .map(|input| input.read(cx).value().to_lowercase())
            .unwrap_or_default();
        library.retain(|preset| preset_name(preset).to_lowercase().contains(&search));
        let card = v_flex()
            .id("current-cloud-eq-editor")
            .w(surface::css(950.))
            .h(window.viewport_size().height - window.rem_size() * (100. / 16.))
            .bg(rgb(0x222222))
            .border_1()
            .border_color(Colors::border())
            .p(surface::css(20.))
            .child(
                h_flex()
                    .justify_between()
                    .child(message(row, "EDIT_EQ_LIST"))
                    .child(
                        razer_widgets::keyboard_navigation::Button::new("current-eq-editor-close")
                            .child("×")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sound.edit_eq_shown = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("current-eq-editor-scroll")
                    .flex_1()
                    .scrollable_y()
                    .gap(surface::css(20.))
                    .child(message(row, "EQ_PRESET_STORAGE_DESC"))
                    .when(self.cloud_thx(), |body| {
                        body.child(div().child(message(row, "EQ_PRESET_THX_WARNING")))
                    })
                    .child(
                        h_flex()
                            .child(message(row, "STORED_EQ_PRESET"))
                            .child(format!(" ({} out of 9)", enabled.len())),
                    )
                    .child(
                        h_flex().flex_wrap().gap(surface::css(10.)).children(
                            enabled
                                .iter()
                                .map(|preset| self.cloud_editor_card(preset, true, cx)),
                        ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .child(message(
                                row,
                                if search.is_empty() {
                                    "EQ_LIBRARY"
                                } else {
                                    "EQ_LIBRARY_SEARCH_RESULTS"
                                },
                            ))
                            .when_some(self.sound.eq_search.as_ref(), |body, input| {
                                body.child(razer_widgets::keyboard_navigation::focus_input(
                                    Input::new(input).w(surface::css(180.)),
                                    input,
                                    cx,
                                ))
                            }),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(surface::css(10.))
                            .children(
                                library
                                    .iter()
                                    .map(|preset| self.cloud_editor_card(preset, false, cx)),
                            )
                            .when(library.is_empty(), |body| {
                                body.child(
                                    v_flex()
                                        .items_center()
                                        .child(
                                            "No presets found. Try searching for something else.",
                                        )
                                        .child(
                                            img(row.media.search_empty.clone())
                                                .object_fit(ObjectFit::Contain)
                                                .size(surface::css(100.)),
                                        ),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                razer_widgets::keyboard_navigation::Button::new(
                                    "current-eq-cancel",
                                )
                                .child(message(row, "CANCEL"))
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.sound.edit_eq_shown = false;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                razer_widgets::keyboard_navigation::Button::new("current-eq-save")
                                    .disabled(!self.sound.edit_eq_changed)
                                    .child(message(row, "SAVE"))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_cloud_editor(cx)),
                                    ),
                            ),
                    )
                    .child(
                        razer_widgets::keyboard_navigation::Button::new("current-eq-reset-all")
                            .child(message(row, "RESET_ALL"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sound.eq_reset = true;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.sound.eq_reset, |body| {
                body.child(
                    v_flex()
                        .p(surface::css(15.))
                        .bg(Colors::background())
                        .child(message(row, "RESET_ALL_EQ"))
                        .child(message(row, "WARNING_EQ_RESET_OVERWRITES_CUSTOM_PROFILES"))
                        .child(
                            h_flex()
                                .gap(surface::css(10.))
                                .child(
                                    razer_widgets::keyboard_navigation::Button::new(
                                        "current-eq-reset-cancel",
                                    )
                                    .child(message(row, "CANCEL"))
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.sound.eq_reset = false;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    razer_widgets::keyboard_navigation::Button::new(
                                        "current-eq-reset-confirm",
                                    )
                                    .child(message(row, "CONFIRM"))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.reset_cloud_oobe(cx)),
                                    ),
                                ),
                        ),
                )
            })
            .when_some(self.sound.eq_remove.as_ref(), |body, preset| {
                let removing = preset.clone();
                body.child(
                    v_flex()
                        .p(surface::css(15.))
                        .bg(Colors::background())
                        .child(message(row, "REMOVE_EQ_DESC"))
                        .child(
                            h_flex()
                                .gap(surface::css(10.))
                                .child(
                                    razer_widgets::keyboard_navigation::Button::new(
                                        "current-eq-remove-cancel",
                                    )
                                    .child(message(row, "CANCEL"))
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.sound.eq_remove = None;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    razer_widgets::keyboard_navigation::Button::new(
                                        "current-eq-remove-confirm",
                                    )
                                    .child(message(row, "REMOVE_EQ"))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            let mut enabled =
                                                list(this.cloud(), "tempEnabledPresets");
                                            enabled.retain(|p| id(p) != id(&removing));
                                            let active = if this.cloud()["tempActivePreset"]
                                                == id(&removing)
                                            {
                                                this.next_cloud_active(&removing, &enabled)
                                            } else {
                                                this.cloud()["tempActivePreset"].clone()
                                            };
                                            let disabled =
                                                list(this.cloud(), "tempDisabledPresets");
                                            this.sound.eq_remove = None;
                                            this.publish_temp_cloud(enabled, disabled, active, cx);
                                        },
                                    )),
                                ),
                        ),
                )
            });
        razer_assets::deferred_resource(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x000000b3))
                .child(card),
        )
        .with_priority(1100)
        .into_any_element()
    }
}
