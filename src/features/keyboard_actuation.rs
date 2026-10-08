//! Current actuation-point editor. Raw mapping units stay in persisted profiles.
//! Sensor feedback, Rapid Trigger and Snap Tap have separate service contracts.
use super::*;
use gpui_kit::base::Button as BaseButton;
use std::collections::BTreeSet;
use std::time::Duration;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    info: Info,
    excluded: Vec<String>,
}
#[derive(Deserialize)]
struct Info {
    min: f32,
    max: f32,
    unit: f32,
    #[serde(rename = "defaultValue")]
    default_value: f32,
    #[serde(rename = "minBreak")]
    min_break: f32,
    #[serde(default, rename = "offSet")]
    offset: f32,
}
#[derive(Deserialize)]
struct Presentation {
    product_id: u32,
    factory_profile_lock: bool,
    sync_padding_x: f32,
    sync_padding_bottom: f32,
    sync_container_height: Option<f32>,
    sync_default_secondary: bool,
}
pub(super) struct State {
    spec: &'static Spec,
    presentation: &'static Presentation,
    make_point: f32,
    sync_all_disabled: bool,
    lifecycle: u64,
    pub(super) selected: BTreeSet<String>,
    slider: Entity<SliderState>,
}

fn presentation(pid: u32) -> &'static Presentation {
    static PRESENTATIONS: OnceLock<Vec<Presentation>> = OnceLock::new();
    PRESENTATIONS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_actuation_presentation_data.json"))
                .expect("statically audited actuation command presentation")
        })
        .iter()
        .find(|presentation| presentation.product_id == pid)
        .expect("audited actuation product presentation")
}

fn specification(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_actuation_data.json"))
                .expect("statically audited actuation specifications")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
}

impl KeyboardProductWorkspace {
    pub(super) fn assign_analog_key(
        &mut self,
        input: &str,
        mut first: Value,
        cx: &mut Context<Self>,
    ) {
        let path = self.mapping_path();
        let (default, release) = self.default_actuation();
        let default_points = json!({"0":default as u32,"1":release as u32});
        let Some(mut mappings) = self.draft.pointer(&path).and_then(Value::as_array).cloned()
        else {
            return;
        };
        let existing = mappings.iter().find(|m| {
            self.mapping_matches_key(m, input)
                && m["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
        });
        first["actuationPoint"] = existing
            .and_then(|m| m.pointer("/mapping/0/actuationPoint"))
            .cloned()
            .unwrap_or(default_points.clone());
        if let Some(rapid) = existing.and_then(|m| m.pointer("/mapping/0/rapidTrigger")) {
            first["rapidTrigger"] = rapid.clone();
        }
        let secondary = existing.and_then(|m| m.pointer("/mapping/1")).cloned()
            .unwrap_or_else(|| json!({"outputType":"defaultGroup","isDefault":false,"isHyperShift":false,"actuationPoint":default_points}));
        mappings.retain(|m| {
            !(self.mapping_matches_key(m, input)
                && m["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift)
        });
        mappings.push(
            json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":self.hypershift,
            "mapping":[first,secondary]}),
        );
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = Value::Array(mappings);
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn reset_analog_assignment(&mut self, input: &str, cx: &mut Context<Self>) {
        let path = self.mapping_path();
        let default = self.default_actuation().0 as u64;
        let Some(mappings) = self.draft.pointer(&path).and_then(Value::as_array) else {
            return;
        };
        let values = mappings.iter().filter_map(|mapping| {
            if !self.mapping_matches_key(mapping, input) || mapping["isHyperShift"].as_bool().unwrap_or(false) != self.hypershift {
                return Some(mapping.clone());
            }
            let points = mapping.pointer("/mapping/0/actuationPoint");
            let rapid = mapping.pointer("/mapping/0/rapidTrigger");
            if points.and_then(|p| p["0"].as_u64().or_else(|| p[0].as_u64())).is_some_and(|v| v != default) || rapid.is_some() {
                let mut next = mapping.clone();
                let mut primary = json!({"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":points});
                if let Some(rapid) = rapid { primary["rapidTrigger"] = rapid.clone(); }
                next["mapping"][0] = primary;
                Some(next)
            } else { None }
        }).collect();
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = Value::Array(values);
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn init_actuation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.spec.pages.iter().any(|page| page == "ACTUATION") {
            return;
        }
        let Some(spec) = specification(self.spec.product_id) else {
            return;
        };
        let info = &spec.info;
        let slider = cx.new(|_| {
            SliderState::new()
                .min((info.min / info.unit).round())
                .max((info.max / info.unit).round())
                .step(1.)
                .default_value((info.default_value / info.unit).round())
        });
        self.subscriptions
            .push(cx.subscribe(&slider, |this, _, event: &SliderEvent, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let Some(state) = &mut this.actuation else {
                        return;
                    };
                    let ids = state.selected.iter().cloned().collect();
                    let raw = value.start() * state.spec.info.unit;
                    state.make_point = raw;
                    state.sync_all_disabled = false;
                    this.apply_actuation(ids, raw, cx);
                }
            }));
        self.actuation = Some(State {
            spec,
            presentation: presentation(spec.product_id),
            make_point: info.default_value,
            sync_all_disabled: true,
            lifecycle: 0,
            selected: BTreeSet::new(),
            slider,
        });
    }

    fn mapping_matches_key(&self, mapping: &Value, input: &str) -> bool {
        mapping["inputID"].as_str() == Some(input)
            || self.spec.keys.iter().any(|key| {
                key["inputID"].as_str() == Some(input)
                    && key["analogInputID"].is_string()
                    && mapping.pointer("/analogInput/Id") == key.get("analogInputID")
            })
    }

    pub(super) fn actuation_key_enabled(&self, input: &str) -> bool {
        let Some(state) = &self.actuation else {
            return false;
        };
        if state.presentation.factory_profile_lock && self.factory_default_profile {
            return false;
        }
        if state.spec.excluded.iter().any(|key| key == input) {
            return false;
        }
        if !self.spec.keys.iter().any(|key| {
            key["inputID"].as_str() == Some(input)
                && key["inputType"] == "AnalogInput"
                && key["isEnabled"].as_bool().unwrap_or(true)
        }) {
            return false;
        }
        !self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)
            .is_some_and(|mappings| {
                mappings
                    .iter()
                    .filter(|mapping| self.mapping_matches_key(mapping, input))
                    .any(|mapping| {
                        let first = mapping.pointer("/mapping/0").unwrap_or(mapping);
                        let secondary = mapping.pointer("/mapping/1");
                        matches!(
                            first["outputType"].as_str(),
                            Some("disableGroup" | "dynamicKeyStrokeGroup")
                        ) || first
                            .pointer("/joystickGroup/isJoystickMovement")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                            || first
                                .pointer("/controllerGroup/controllerMode/isAnalog")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            || secondary.is_some_and(|second| {
                                second["outputType"]
                                    .as_str()
                                    .and_then(|kind| second.get(kind))
                                    .and_then(Value::as_object)
                                    .is_some_and(|group| !group.is_empty())
                            })
                    })
            })
    }

    fn default_actuation(&self) -> (f32, f32) {
        let info = &self.actuation.as_ref().expect("actuation page").spec.info;
        let config = &self.spec.config["DeviceInfo"]["analogSpecs"];
        let high = (self.draft["guid"] == "177a20d5-f30a-4a52-923e-4fadb7db8392")
            .then(|| &config["actuationHighSensitivityInfo"]);
        let value = high
            .and_then(|v| v["defaultValue"].as_f64())
            .map(|v| v as f32)
            .unwrap_or(info.default_value);
        let unit = high
            .and_then(|v| v["unit"].as_f64())
            .map(|v| v as f32)
            .unwrap_or(info.unit);
        let min_break = high
            .and_then(|v| v["minBreak"].as_f64())
            .map(|v| v as f32)
            .unwrap_or(info.min_break);
        (value, (value - unit).max(min_break))
    }

    pub(super) fn select_actuation_key(
        &mut self,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.actuation_key_enabled(input) {
            return;
        }
        let raw = self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)
            .and_then(|mappings| {
                mappings
                    .iter()
                    .find(|m| self.mapping_matches_key(m, input) && m["isHyperShift"] != true)
            })
            .and_then(|m| m.pointer("/mapping/0/actuationPoint/0"))
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .unwrap_or(self.default_actuation().0);
        let state = self.actuation.as_mut().unwrap();
        state.make_point = raw;
        if !state.selected.remove(input) {
            state.selected.insert(input.to_owned());
        }
        state.sync_all_disabled = state.selected.is_empty();
        let slider = state.slider.clone();
        let value = (raw / state.spec.info.unit).round();
        self.syncing = true;
        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        self.syncing = false;
        cx.notify();
    }

    fn apply_actuation(&mut self, ids: Vec<String>, raw: f32, cx: &mut Context<Self>) {
        self.apply_actuation_points(ids, raw, false, cx);
    }

    fn apply_actuation_points(
        &mut self,
        ids: Vec<String>,
        raw: f32,
        synchronize: bool,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| self.actuation_key_enabled(id))
            .collect();
        if ids.is_empty() || !raw.is_finite() {
            return;
        }
        let Some(state) = &self.actuation else {
            return;
        };
        let info = &state.spec.info;
        let sync_default_secondary = synchronize && state.presentation.sync_default_secondary;
        let raw = if synchronize {
            // The source sends makeActuationPointValue itself. Some first
            // generation defaults are not multiples of a slider step.
            raw as u32
        } else {
            ((raw / info.unit).round().clamp(
                (info.min / info.unit).round(),
                (info.max / info.unit).round(),
            ) * info.unit) as u32
        };
        let (default_value, release) = self.default_actuation();
        // Current syncActuationKeys sends identical make/release thresholds;
        // ordinary make-point editing retains its separate release threshold.
        let points = json!({"0":raw,"1":if synchronize { raw } else { release as u32 }});
        let path = self.mapping_path();
        let Some(mut mappings) = self.draft.pointer(&path).and_then(Value::as_array).cloned()
        else {
            return;
        };
        for id in ids {
            let mut found = false;
            for mapping in mappings
                .iter_mut()
                .filter(|m| self.mapping_matches_key(m, &id))
            {
                found = true;
                // Older local drafts used flat output groups. Preserve their
                // assignment while adopting the source's analog mapping shape.
                if !mapping["mapping"].is_array() {
                    let mut first = json!({"outputType":mapping["outputType"]});
                    if let Some(kind) = mapping["outputType"].as_str() {
                        if let Some(group) = mapping.get(kind) {
                            first[kind] = group.clone();
                        }
                    }
                    *mapping = json!({"inputID":id,"inputType":"AnalogInput",
                        "isHyperShift":mapping["isHyperShift"].as_bool().unwrap_or(false),
                        "mapping":[first,{"outputType":"defaultGroup","isDefault":false,"isHyperShift":false,
                            "actuationPoint":{"0":default_value as u32,"1":release as u32}}]});
                }
                if let Some(first) = mapping
                    .pointer_mut("/mapping/0")
                    .and_then(Value::as_object_mut)
                {
                    first.insert("actuationPoint".into(), points.clone());
                }
                if sync_default_secondary
                    && mapping
                        .pointer("/mapping/1/outputType")
                        .and_then(Value::as_str)
                        == Some("defaultGroup")
                {
                    mapping["mapping"][1]["actuationPoint"] = points.clone();
                }
            }
            if !found && raw != default_value as u32 {
                for shift in [false, true] {
                    mappings.push(json!({"inputID":id,"inputType":"AnalogInput","isHyperShift":shift,
                        "mapping":[{"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":points},
                            {"outputType":"defaultGroup","isDefault":false,"isHyperShift":false,
                                "actuationPoint":if sync_default_secondary { points.clone() } else {
                                    json!({"0":default_value as u32,"1":release as u32})
                                }}]}));
                }
            }
        }
        if let Some(target) = self.draft.pointer_mut(&path) {
            let value = Value::Array(mappings);
            if *target != value {
                *target = value;
                cx.emit(KeyboardProductChanged);
            }
        }
        cx.notify();
    }

    fn sync_actuation(&mut self, all: bool, cx: &mut Context<Self>) {
        let Some(state) = &self.actuation else {
            return;
        };
        if state.presentation.factory_profile_lock && self.factory_default_profile
            || if all {
                state.sync_all_disabled
            } else {
                state.selected.is_empty()
            }
        {
            return;
        }
        let raw = state.make_point;
        let lifecycle = state.lifecycle;
        let has_mappings = self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)
            .is_some_and(|mappings| !mappings.is_empty());
        let ids = if all {
            self.spec
                .keys
                .iter()
                .filter_map(|key| key["inputID"].as_str().map(str::to_owned))
                .collect()
        } else {
            state.selected.iter().cloned().collect()
        };
        self.apply_actuation_points(ids, raw, true, cx);
        if all && has_mappings {
            // Source disables Sync all after 2500ms. Scope its delayed view
            // state to this profile/page rather than mutating a restored owner.
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(state) = &mut this.actuation {
                        if state.lifecycle == lifecycle && this.page == "ACTUATION" {
                            state.sync_all_disabled = true;
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
    }

    pub(super) fn clear_actuation_selection(&mut self) {
        if let Some(state) = &mut self.actuation {
            state.selected.clear();
            state.sync_all_disabled = true;
            state.lifecycle = state.lifecycle.wrapping_add(1);
        }
    }

    pub(super) fn actuation_page(&self, cx: &Context<Self>) -> AnyElement {
        let Some(state) = &self.actuation else {
            return surface::panel(t("ACTUATION"), cx).into_any_element();
        };
        let info = &state.spec.info;
        let locked = state.presentation.factory_profile_lock && self.factory_default_profile;
        let value = state.slider.read(cx).value().start();
        let display = |value: f32| format!("{:.1} mm", value / 10. + info.offset);
        let keys = h_flex()
            .gap_2()
            .child(
                Button::new("actuation-select-all")
                    .label(t("SELECT_ALL_KEYS"))
                    .outline()
                    .disabled(locked)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let selected = this
                            .spec
                            .keys
                            .iter()
                            .filter_map(|key| key["inputID"].as_str())
                            .filter(|id| this.actuation_key_enabled(id))
                            .map(str::to_owned)
                            .collect();
                        if let Some(state) = &mut this.actuation {
                            state.selected = selected;
                            state.sync_all_disabled = state.selected.is_empty();
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("actuation-deselect-all")
                    .label(t("DESELECT_ALL_KEYS"))
                    .outline()
                    .disabled(locked || state.selected.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.actuation {
                            state.selected.clear();
                            state.sync_all_disabled = true;
                        }
                        cx.notify();
                    })),
            );
        let mut panel = surface::panel(t("ACTUATION_POINT"), cx)
            .child(div().child(t("ACTUATION_POINT_TITLE_DESC")))
            .child(div().child(display(value)))
            .child(Slider::new(&state.slider).disabled(locked || state.selected.is_empty()))
            .child(
                h_flex()
                    .justify_between()
                    .child(display((info.min / info.unit).round()))
                    .child(display((info.max / info.unit).round())),
            );
        let analog_v2 = self.spec.config["DeviceInfo"]["AnalogGenVersion"]
            .as_str()
            .is_some_and(|generation| generation == "analogV2");
        if analog_v2 && value < 10. {
            panel = panel.child(
                div()
                    .mt(surface::css(20.))
                    .text_color(crate::ui::theme::KeyboardActuationColors::warning())
                    .child(t("ACTUATION_WARINING")),
            );
        }
        let selected_disabled = locked || state.selected.is_empty();
        let all_disabled = locked || state.sync_all_disabled;
        let sync_button = |id: &'static str,
                           label: &'static str,
                           disabled: bool,
                           all: bool,
                           cx: &Context<Self>| {
            use crate::ui::theme::KeyboardActuationColors as Colors;
            let label = t(label);
            h_flex()
                .w(relative(0.5))
                .flex_shrink_0()
                .items_center()
                .font_family("Roboto")
                .pt(surface::css(10.))
                .pb(surface::css(state.presentation.sync_padding_bottom))
                .px(surface::css(state.presentation.sync_padding_x))
                .opacity(if disabled { 0.3 } else { 1. })
                .child(
                    BaseButton::new(id)
                        .accessibility_label(label.clone())
                        .disabled(disabled)
                        .w(surface::css(35.))
                        .h(surface::css(25.))
                        .mr(surface::css(10.))
                        .flex_shrink_0()
                        .relative()
                        .overflow_hidden()
                        .border_1()
                        .border_color(Colors::sync_border())
                        .rounded(surface::css(3.))
                        .bg(Colors::sync_background())
                        .when(!disabled, |button| {
                            button.hover(|style| style.border_color(Colors::sync_hover()))
                        })
                        .focus_visible(|style| style.border_color(Colors::sync_hover()))
                        .child(
                            img("synapse/keyboard-actuation-sync.svg")
                                .absolute()
                                .left(surface::css(7.))
                                .top(surface::css(-5.))
                                // CSS background-size is auto: preserve the
                                // source SVG's full 210×33 sprite, then clip.
                                .w(surface::css(210.))
                                .h(surface::css(33.)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.sync_actuation(all, cx);
                        })),
                )
                .child(
                    div()
                        .h(surface::css(27.))
                        .min_w(surface::css(90.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(surface::css(3.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(14.))
                        .text_color(Colors::sync_text())
                        .underline()
                        .child(label),
                )
        };
        panel = panel.child(
            h_flex()
                .w_full()
                .items_start()
                .when_some(
                    state.presentation.sync_container_height,
                    |container, height| container.h(surface::css(height)),
                )
                .child(sync_button(
                    "actuation-sync-all",
                    "SYNC_SETTINGS_TO_ALL_KEYS",
                    all_disabled,
                    true,
                    cx,
                ))
                .child(sync_button(
                    "actuation-sync-selected",
                    "SYNC_SETTINGS_TO_SELECTED_KEYS",
                    selected_disabled,
                    false,
                    cx,
                )),
        );
        v_flex()
            .gap_5()
            .child(self.keyboard_image(true, cx))
            .child(keys)
            .child(panel)
            .into_any_element()
    }
}
