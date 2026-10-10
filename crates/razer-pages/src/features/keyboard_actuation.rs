//! Current actuation-point editor. Raw mapping units stay in persisted profiles.
//! Sensor feedback, Rapid Trigger and Snap Tap have separate service contracts.
use super::*;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::tooltip::Tooltip;
use std::collections::BTreeSet;
use std::time::Duration;

fn rapid_help_control(id: &'static str, text: SharedString) -> AnyElement {
    BaseButton::new(id)
        .w(surface::css(14.))
        .h(surface::css(14.))
        .flex_shrink_0()
        .p_0()
        .rounded_full()
        .bg(razer_widgets::theme::TooltipColors::help_background())
        .accessibility_label(text.clone())
        .child(img("synapse/automation-tooltip_questionmark.svg").size_full())
        .tooltip(move |window, cx| {
            let text = text.clone();
            Tooltip::element(move |_, cx| {
                v_flex()
                    .max_w(surface::css(300.))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(razer_widgets::theme::TooltipColors::border())
                    .bg(razer_widgets::theme::TooltipColors::background())
                    .text_color(razer_widgets::theme::TooltipColors::foreground())
                    .child(div().child(text.clone()))
                    .child(
                        img("synapse/rapid-trigger-tooltip-slider.gif")
                            .w(surface::css(200.))
                            .h_auto(),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(surface::css(20.))
                            .child(t("TRADITIONAL_SWITCH"))
                            .child(t("RAPID_TRIGGER_BREAK_LINE")),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(surface::css(8.))
                            .mt(surface::css(10.))
                            .child(t("LEGEND"))
                            .child(
                                h_flex()
                                    .gap(surface::css(8.))
                                    .text_color(cx.theme().primary)
                                    .child(
                                        div()
                                            .size(surface::css(14.))
                                            .rounded_full()
                                            .bg(cx.theme().primary),
                                    )
                                    .child(t("ACTIVE")),
                            )
                            .child(
                                h_flex()
                                    .gap(surface::css(8.))
                                    .text_color(cx.theme().danger)
                                    .child(
                                        div()
                                            .size(surface::css(14.))
                                            .rounded_full()
                                            .bg(cx.theme().danger),
                                    )
                                    .child(t("RESET_LOWERCASE")),
                            ),
                    )
            })
            .build(window, cx)
        })
        .into_any_element()
}

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
    selection_history: Vec<String>,
    slider: Entity<SliderState>,
    /// Rapid Trigger controls are part of the ACTUATION route for analog
    /// keyboards (the source calls this the `$h` panel). Values are retained
    /// in the mapping's `rapidTrigger` object as the raw device units.
    pub(super) rapid_enabled: bool,
    pub(super) rapid_upstroke: bool,
    pub(super) rapid_make: Entity<SliderState>,
    pub(super) rapid_break: Entity<SliderState>,
    rapid_continuous: bool,
    continuous_enabled: bool,
    reset_popup: bool,
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
        self.request_actuation_mapping(cx);
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
        self.request_actuation_mapping(cx);
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
        let rapid_default = Self::rapid_display(
            if self.spec.config["DeviceInfo"]["AnalogGenVersion"] == "analogV1" {
                4.
            } else {
                10.
            },
        );
        let rapid_make = cx.new(|_| {
            SliderState::new()
                .min(0.1)
                .max(1.0)
                .step(0.1)
                .default_value(rapid_default)
        });
        let rapid_break = cx.new(|_| {
            SliderState::new()
                .min(0.1)
                .max(1.0)
                .step(0.1)
                .default_value(0.1)
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
        self.subscriptions.push(
            cx.subscribe(&rapid_make, |this, _, event: &SliderEvent, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let v = value.start().clamp(0.1, 1.0);
                    if let Some(state) = &mut this.actuation {
                        let ids = state.selected.iter().cloned().collect::<Vec<_>>();
                        this.apply_rapid(ids, Some(v), None, None, cx);
                    }
                }
            }),
        );
        self.subscriptions.push(
            cx.subscribe(&rapid_break, |this, _, event: &SliderEvent, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let v = value.start().clamp(0.1, 1.0);
                    if let Some(state) = &this.actuation {
                        let ids = state.selected.iter().cloned().collect::<Vec<_>>();
                        this.apply_rapid(ids, None, Some(v), None, cx);
                    }
                }
            }),
        );
        self.actuation = Some(State {
            spec,
            presentation: presentation(spec.product_id),
            make_point: info.default_value,
            sync_all_disabled: true,
            lifecycle: 0,
            selected: BTreeSet::new(),
            selection_history: Vec::new(),
            slider,
            rapid_enabled: false,
            rapid_upstroke: false,
            rapid_make,
            rapid_break,
            rapid_continuous: false,
            continuous_enabled: false,
            reset_popup: false,
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
        // Current actuation reducer initializes both default thresholds to
        // defaultValue. minBreak constrains custom release, not this default.
        let _ = (unit, min_break);
        (value, value)
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
        let state = self.actuation.as_mut().unwrap();
        if state.selected.remove(input) {
            state.selection_history.retain(|id| id != input);
        } else {
            state.selected.insert(input.to_owned());
            state.selection_history.push(input.to_owned());
        }
        self.refresh_actuation_selection(window, cx);
    }

    /// Source Wh/Jh choose the last individually selected key; bulk selections
    /// choose their most common value. Never mutate mappings while refreshing.
    pub(super) fn refresh_actuation_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &self.actuation else {
            return;
        };
        let history = state.selection_history.last().cloned();
        let ids: Vec<_> = self
            .spec
            .keys
            .iter()
            .filter_map(|key| key["inputID"].as_str())
            .filter(|id| state.selected.contains(*id))
            .map(str::to_owned)
            .collect();
        let mappings = self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array);
        let primary = |id: &str| {
            mappings
                .and_then(|items| {
                    items.iter().find(|mapping| {
                        self.mapping_matches_key(mapping, id)
                            && !mapping["isHyperShift"].as_bool().unwrap_or(false)
                    })
                })
                .and_then(|mapping| mapping.pointer("/mapping/0"))
        };
        let choose = |rapid: bool| -> Option<Value> {
            if let Some(id) = &history {
                // An unmapped last key shows defaults, including when some
                // earlier selected key has Rapid Trigger enabled.
                return primary(id)
                    .and_then(|p| {
                        p.get(if rapid {
                            "rapidTrigger"
                        } else {
                            "actuationPoint"
                        })
                    })
                    .cloned();
            }
            let mut groups: Vec<(String, usize, Option<Value>)> = vec![];
            for id in &ids {
                let value = primary(id)
                    .and_then(|p| {
                        p.get(if rapid {
                            "rapidTrigger"
                        } else {
                            "actuationPoint"
                        })
                    })
                    .cloned();
                let signature = if rapid {
                    value
                        .as_ref()
                        .filter(|v| v["continuous"].as_bool().unwrap_or(false))
                        .map(|v| {
                            format!(
                                "{}:{}",
                                v["makeSensitivity"],
                                if v["isEnableUpStroke"] == true {
                                    v["breakSensitivity"].clone()
                                } else {
                                    Value::Null
                                }
                            )
                        })
                        .unwrap_or_else(|| "disabled".into())
                } else {
                    value
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_else(|| "disabled".into())
                };
                if let Some(group) = groups.iter_mut().find(|g| g.0 == signature) {
                    group.1 += 1;
                    group.2 = value;
                } else {
                    groups.push((signature, 1, value));
                }
            }
            let mut best: Option<(usize, Option<Value>)> = None;
            for (_, count, value) in groups {
                if best
                    .as_ref()
                    .is_none_or(|(max, _)| count > *max || (!rapid && count == *max))
                {
                    best = Some((count, value));
                }
            }
            best.and_then(|(_, value)| value)
        };
        let points = choose(false);
        let rapid = choose(true);
        let default = self.default_actuation().0;
        let raw = points
            .as_ref()
            .and_then(|p| p["0"].as_f64().or_else(|| p[0].as_f64()))
            .map(|v| v as f32)
            .unwrap_or(default);
        let rapid_default = if self.spec.config["DeviceInfo"]["AnalogGenVersion"] == "analogV1" {
            4.
        } else {
            10.
        };
        let make = Self::rapid_display(
            rapid
                .as_ref()
                .and_then(|r| r["makeSensitivity"].as_f64())
                .unwrap_or(rapid_default),
        );
        let release = Self::rapid_display(
            rapid
                .as_ref()
                .and_then(|r| r["breakSensitivity"].as_f64())
                .unwrap_or(10.),
        );
        let state = self.actuation.as_mut().unwrap();
        state.make_point = raw;
        state.sync_all_disabled = ids.is_empty();
        state.rapid_enabled = rapid.is_some() && !ids.is_empty();
        state.rapid_upstroke = rapid
            .as_ref()
            .is_some_and(|r| r["isEnableUpStroke"] == true);
        state.rapid_continuous = rapid.as_ref().is_some_and(|r| r["continuous"] == true);
        let slider = state.slider.clone();
        let rapid_make = state.rapid_make.clone();
        let rapid_break = state.rapid_break.clone();
        let value = (raw / state.spec.info.unit).round();
        self.syncing = true;
        slider.update(cx, |s, cx| s.set_value(value, window, cx));
        rapid_make.update(cx, |s, cx| s.set_value(make, window, cx));
        rapid_break.update(cx, |s, cx| s.set_value(release, window, cx));
        self.syncing = false;
        cx.notify();
    }

    fn rapid_display(raw: f64) -> f32 {
        ((1.1 - (raw as f32 / 10.).min(1.)) * 10.).round() / 10.
    }

    fn rapid_raw(value: f32) -> u32 {
        (10. * (1.1 - value.clamp(0.1, 1.))).round() as u32
    }

    fn apply_rapid(
        &mut self,
        ids: Vec<String>,
        make: Option<f32>,
        break_value: Option<f32>,
        enabled: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| self.actuation_key_enabled(id))
            .collect();
        if ids.is_empty() {
            return;
        }
        let Some(state) = &self.actuation else {
            return;
        };
        let active = enabled.unwrap_or(state.rapid_enabled);
        let make_raw = make
            .map(Self::rapid_raw)
            .unwrap_or_else(|| Self::rapid_raw(state.rapid_make.read(cx).value().start()));
        let release_raw = if state.rapid_upstroke {
            break_value
                .map(Self::rapid_raw)
                .unwrap_or_else(|| Self::rapid_raw(state.rapid_break.read(cx).value().start()))
        } else {
            make_raw
        };
        let rapid = json!({"makeSensitivity":make_raw,"breakSensitivity":release_raw,"isEnableUpStroke":state.rapid_upstroke});
        self.apply_rapid_mapping(ids, active, rapid.clone(), cx);
        // Source dAp also publishes this global sensitivity observation.
        self.request_actuation_message(
            json!({"type":"ON_SYNC_GLOBAL_RAPID_TRIGGER","payload":{"value":rapid}}),
            cx,
        );
        self.request_actuation_mapping(cx);
        if let Some(state) = &mut self.actuation {
            state.rapid_enabled = active;
        }
        cx.notify();
    }

    fn apply_rapid_mapping(
        &mut self,
        ids: Vec<String>,
        enabled: bool,
        rapid: Value,
        cx: &mut Context<Self>,
    ) {
        let path = self.mapping_path();
        let (default, release) = self.default_actuation();
        let Some(mut mappings) = self.draft.pointer(&path).and_then(Value::as_array).cloned()
        else {
            return;
        };
        for id in ids {
            let mut found = false;
            for mapping in mappings
                .iter_mut()
                .filter(|mapping| self.mapping_matches_key(mapping, &id))
            {
                found = true;
                let Some(primary) = mapping
                    .pointer_mut("/mapping/0")
                    .and_then(Value::as_object_mut)
                else {
                    continue;
                };
                if enabled {
                    primary.insert("rapidTrigger".into(), rapid.clone());
                } else {
                    primary.remove("rapidTrigger");
                }
            }
            if !found && enabled {
                for shift in [false, true] {
                    mappings.push(json!({"inputID":id,"inputType":"AnalogInput","isHyperShift":shift,
                        "mapping":[{"outputType":"keyboardGroup","keyboardGroup":{},
                            "actuationPoint":{"0":default as u32,"1":release as u32},"rapidTrigger":rapid},
                            {"outputType":"defaultGroup","isDefault":false,"isHyperShift":false,
                                "actuationPoint":{"0":default as u32,"1":release as u32}}]}));
                }
            }
        }
        // generateAnalogMappingList removes now-default analog entries.
        mappings.retain(|mapping| {
            if mapping["inputType"] != "AnalogInput" {
                return true;
            }
            let first = &mapping["mapping"][0];
            let secondary = &mapping["mapping"][1];
            let assigned = |p: &Value| {
                p["outputType"]
                    .as_str()
                    .and_then(|kind| p.get(kind))
                    .and_then(Value::as_object)
                    .is_some_and(|group| !group.is_empty())
            };
            first["outputType"] == "disableGroup"
                || assigned(first)
                || assigned(secondary)
                || first.get("rapidTrigger").is_some()
                || first["actuationPoint"]["0"]
                    .as_u64()
                    .is_some_and(|v| v != default as u64)
        });
        if let Some(target) = self.draft.pointer_mut(&path) {
            let value = Value::Array(mappings);
            if *target != value {
                *target = value;
                cx.emit(KeyboardProductChanged);
            }
        }
    }

    pub(super) fn toggle_rapid(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let ids = self
            .actuation
            .as_ref()
            .map(|s| s.selected.iter().cloned().collect())
            .unwrap_or_default();
        self.apply_rapid(ids, None, None, Some(enabled), cx);
    }

    pub(super) fn toggle_rapid_upstroke(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let ids = self
            .actuation
            .as_ref()
            .map(|s| s.selected.iter().cloned().collect())
            .unwrap_or_default();
        if let Some(state) = &mut self.actuation {
            state.rapid_upstroke = enabled;
        }
        self.apply_rapid(ids, None, None, None, cx);
    }

    fn sync_rapid(&mut self, all: bool, cx: &mut Context<Self>) {
        let Some(state) = &self.actuation else {
            return;
        };
        if !state.rapid_enabled || state.selected.is_empty() {
            return;
        }
        let rapid = json!({"makeSensitivity":Self::rapid_raw(state.rapid_make.read(cx).value().start()),
            "breakSensitivity":Self::rapid_raw(state.rapid_break.read(cx).value().start()),
            "continuous":state.rapid_continuous,"isEnableUpStroke":state.rapid_upstroke});
        let ids = if all {
            self.spec
                .keys
                .iter()
                .filter_map(|key| key["inputID"].as_str())
                .filter(|id| self.actuation_key_enabled(id))
                .map(str::to_owned)
                .collect()
        } else {
            state.selected.iter().cloned().collect()
        };
        self.apply_rapid_mapping(ids, true, rapid, cx);
        self.request_actuation_mapping(cx);
        cx.notify();
    }

    fn continuous_supported(&self) -> bool {
        // The current ACTUATION roots render the separate global widget for
        // these products. 580/614/642 render only per-key Rapid Trigger.
        matches!(
            self.spec.product_id,
            678 | 679 | 688 | 719 | 720 | 721 | 728 | 740 | 741 | 742 | 746 | 747
        )
    }

    fn toggle_continuous(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !self.continuous_supported()
            || self.factory_default_profile
                && self
                    .actuation
                    .as_ref()
                    .is_some_and(|s| s.presentation.factory_profile_lock)
        {
            return;
        }
        if let Some(state) = &mut self.actuation {
            state.continuous_enabled = enabled;
        }
        // This reducer is device state, independent of profile key mappings.
        self.request_actuation_message(
            json!({"type":"ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER","payload":{"value":enabled}}),
            cx,
        );
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
        self.request_actuation_mapping(cx);
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

    /// 679 Wh.resetActuationToDefault -> 62905.l -> Ja. This resets the
    /// actuation thresholds while retaining assignments/rapid trigger exactly
    /// as the source reducer does; it is not magnetic sensor calibration.
    fn reset_679_actuation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 679 || self.factory_default_profile {
            return;
        }
        let default = self.default_actuation().0 as u32;
        let preserve_hypershift = self.spec.config["DeviceInfo"]["resetActuationWithHypershift"]
            .as_bool()
            .unwrap_or(false);
        let path = self.mapping_path();
        let Some(values) = self.draft.pointer(&path).and_then(Value::as_array) else {
            return;
        };
        let mappings = values
            .iter()
            .filter_map(|mapping| {
                if !mapping["mapping"].is_array() {
                    return Some(mapping.clone());
                }
                let first = &mapping["mapping"][0];
                let secondary = &mapping["mapping"][1];
                let first_default = first["actuationPoint"]["0"]
                    .as_u64()
                    .or_else(|| first["actuationPoint"][0].as_u64())
                    == Some(default as u64);
                let preserve_secondary = if secondary["outputType"] != "defaultGroup" {
                    !preserve_hypershift || first_default
                } else if secondary.get("joystickGroup").is_some() {
                    secondary["joystickGroup"]["isJoystickMovement"]
                        .as_bool()
                        .unwrap_or(false)
                } else {
                    secondary["controllerGroup"]["controllerMode"]["isAnalog"]
                        .as_bool()
                        .unwrap_or(false)
                };
                if preserve_secondary {
                    return Some(mapping.clone());
                }
                let assigned = first["outputType"]
                    .as_str()
                    .and_then(|kind| first.get(kind))
                    .and_then(Value::as_object)
                    .is_some_and(|group| !group.is_empty());
                if first.get("rapidTrigger").is_none()
                    && first["outputType"] != "disableGroup"
                    && !(preserve_hypershift && !first_default)
                    && !assigned
                {
                    return None;
                }
                let mut next = mapping.clone();
                if next["mapping"][0]["actuationPoint"].is_array() {
                    next["mapping"][0]["actuationPoint"][0] = json!(default);
                } else {
                    next["mapping"][0]["actuationPoint"]["0"] = json!(default);
                }
                Some(next)
            })
            .collect();
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = Value::Array(mappings);
        }
        if let Some(state) = &mut self.actuation {
            state.reset_popup = false;
            state.selected.clear();
            state.selection_history.clear();
        }
        self.refresh_actuation_selection(window, cx);
        cx.emit(KeyboardProductChanged);
        self.request_actuation_mapping(cx);
    }

    pub(super) fn clear_actuation_selection(&mut self) {
        self.cancel_actuation_connection();
        if let Some(state) = &mut self.actuation {
            state.selected.clear();
            state.selection_history.clear();
            state.sync_all_disabled = true;
            state.rapid_enabled = false;
            state.rapid_upstroke = false;
            state.rapid_continuous = false;
            state.lifecycle = state.lifecycle.wrapping_add(1);
            state.reset_popup = false;
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
            .justify_center()
            .gap(surface::css(10.))
            .mt(surface::css(30.))
            .mb(surface::css(10.))
            .child(
                BaseButton::new("actuation-select-all")
                    .child(t("SELECT_ALL_KEYS"))
                    .h(surface::css(27.))
                    .px(surface::css(16.))
                    .border_1()
                    .border_color(rgb(0x9c9c9c))
                    .rounded(surface::css(3.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .disabled(
                        locked
                            || self
                                .spec
                                .keys
                                .iter()
                                .filter_map(|key| key["inputID"].as_str())
                                .filter(|id| self.actuation_key_enabled(id))
                                .all(|id| state.selected.contains(id)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
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
                            state.selection_history.clear();
                            state.sync_all_disabled = state.selected.is_empty();
                        }
                        this.refresh_actuation_selection(window, cx);
                    })),
            )
            .child(
                BaseButton::new("actuation-deselect-all")
                    .child(t("DESELECT_ALL_KEYS"))
                    .h(surface::css(27.))
                    .px(surface::css(16.))
                    .border_1()
                    .border_color(rgb(0x9c9c9c))
                    .rounded(surface::css(3.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .disabled(locked || state.selected.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(state) = &mut this.actuation {
                            state.selected.clear();
                            state.selection_history.clear();
                            state.sync_all_disabled = true;
                        }
                        this.refresh_actuation_selection(window, cx);
                    })),
            );
        // Current 740/746 choose the magnetic animation and 742 the low-profile
        // animation. Their CSS fixes height to 140px, removes bottom padding,
        // and overrides the ordinary animation's margins.
        let magnetic = self.spec.config["DeviceInfo"]["isMagneticSwitch"] == true;
        let low_profile = self.spec.config["DeviceInfo"]["isLowProfileSwitch"] == true;
        let (animation, layout_height, margin_top, margin_bottom, paint_top, paint_height) =
            if magnetic {
                (
                    "synapse/keyboard-magnetic-actuation.gif",
                    140.,
                    40.,
                    10.,
                    -35.,
                    210.,
                )
            } else if low_profile {
                (
                    "synapse/keyboard-low-profile-actuation.gif",
                    140.,
                    70.,
                    -20.,
                    -35.,
                    210.,
                )
            } else {
                (
                    "synapse/keyboard-actuation.gif",
                    171.25,
                    -30.,
                    10.,
                    -12.8125,
                    196.875,
                )
            };
        let mut panel = surface::panel(t("ACTUATION_POINT"), cx)
            .relative()
            .child(
                div()
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .child(surface::help_control(
                        "keyboard-actuation-help",
                        t("ACTUATION_POINT_TOOLTIP"),
                    )),
            )
            .child(div().child(t("ACTUATION_POINT_TITLE_DESC")))
            .child(
                h_flex()
                    .items_start()
                    .mb(surface::css(65.))
                    .pl(surface::css(75.))
                    .child(
                        div()
                            // Preserve the CSS border box, then paint its
                            // content scaled 1.5 around the wrapper centre.
                            .relative()
                            .w(surface::css(140.))
                            .h(surface::css(layout_height))
                            .mt(surface::css(margin_top))
                            .mx(surface::css(10.))
                            .mb(surface::css(margin_bottom))
                            .child(
                                img(animation)
                                    .absolute()
                                    .left(surface::css(-5.))
                                    .top(surface::css(paint_top))
                                    .w(surface::css(150.))
                                    .h(surface::css(paint_height))
                                    .object_fit(ObjectFit::Fill),
                            ),
                    )
                    .child(
                        Slider::new(&state.slider)
                            .vertical()
                            .w(surface::css(70.))
                            .h(surface::css(204.))
                            .disabled(locked || state.selected.is_empty()),
                    ),
            )
            .child(div().child(display(value)))
            .child(
                h_flex()
                    .justify_between()
                    .child(display((info.min / info.unit).round()))
                    .child(display((info.max / info.unit).round())),
            );
        if self.spec.product_id == 679 {
            panel = panel.child(
                div()
                    .absolute()
                    .right(surface::css(36.))
                    .top(surface::css(31.))
                    .size(surface::css(20.))
                    .child(
                        BaseButton::new("679-actuation-reset-open")
                            .size_full()
                            .disabled(locked)
                            .child(img("synapse/monitor-icon_refresh.80aa16c3.svg").size_full())
                            .tooltip(|window, cx| {
                                Tooltip::new(t("RESET_TO_DEFAULT")).build(window, cx)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(state) = &mut this.actuation {
                                    state.reset_popup = true;
                                }
                                cx.notify();
                            })),
                    )
                    .when(state.reset_popup, |control| {
                        control.child(
                            v_flex()
                                .id("679-actuation-reset-popup")
                                .absolute()
                                .right(surface::css(-55.))
                                .top(surface::css(25.))
                                .min_w(surface::css(300.))
                                .p(surface::css(20.))
                                .gap(surface::css(10.))
                                .items_center()
                                .bg(rgb(0x111111))
                                .border(surface::css(2.))
                                .border_color(rgb(0xff4500))
                                .rounded(surface::css(4.))
                                .child(
                                    div()
                                        .text_color(rgb(0xff4500))
                                        .font_weight(FontWeight::BOLD)
                                        .child(t("RESET_ACTUATION").to_uppercase()),
                                )
                                .child(div().text_center().child(t("RESET_ACTUATION_POPUP_DESC")))
                                .child(
                                    BaseButton::new("679-actuation-reset-confirm")
                                        .px(surface::css(16.))
                                        .py(surface::css(6.))
                                        .rounded(surface::css(3.))
                                        .bg(rgb(0xff4500))
                                        .text_color(rgb(0))
                                        .child(t("RESET"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reset_679_actuation(window, cx)
                                        })),
                                )
                                .on_mouse_down_out(cx.listener(
                                    |this, _: &MouseDownEvent, _, cx| {
                                        if let Some(state) = &mut this.actuation {
                                            state.reset_popup = false;
                                        }
                                        cx.notify();
                                    },
                                )),
                        )
                    }),
            );
        }
        let analog_v2 = self.spec.config["DeviceInfo"]["AnalogGenVersion"]
            .as_str()
            .is_some_and(|generation| generation == "analogV2");
        if analog_v2 && value < 10. {
            panel = panel.child(
                div()
                    .mt(surface::css(20.))
                    .text_color(razer_widgets::theme::KeyboardActuationColors::warning())
                    .child(t("ACTUATION_WARINING")),
            );
        }
        let selected_disabled = locked || state.selected.is_empty();
        let all_disabled = locked || state.sync_all_disabled;
        let sync_button = |id: &'static str,
                           label: &'static str,
                           disabled: bool,
                           all: bool,
                           rapid: bool,
                           cx: &Context<Self>| {
            use razer_widgets::theme::KeyboardActuationColors as Colors;
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
                            if rapid {
                                this.sync_rapid(all, cx);
                            } else {
                                this.sync_actuation(all, cx);
                            }
                        })),
                )
                .child(
                    BaseButton::new(SharedString::from(format!("{id}-label")))
                        .accessibility_label(label.clone())
                        .disabled(disabled)
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
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if rapid {
                                this.sync_rapid(all, cx);
                            } else {
                                this.sync_actuation(all, cx);
                            }
                        })),
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
                    false,
                    cx,
                ))
                .child(sync_button(
                    "actuation-sync-selected",
                    "SYNC_SETTINGS_TO_SELECTED_KEYS",
                    selected_disabled,
                    false,
                    false,
                    cx,
                )),
        );
        let rapid_disabled = locked || state.selected.is_empty();
        let inactive = rapid_disabled || !state.rapid_enabled;
        let endpoints = || {
            h_flex()
                .justify_between()
                .child(format!("1.0 mm ({})", t("LOW")))
                .child(format!("0.1 mm ({})", t("HIGH")))
        };
        let mut rapid = surface::panel_with_title_switch(
            t("RAPID_TRIGGER_HEADER"),
            surface::SynapseSwitch::new("keyboard-rapid-trigger")
                .checked(state.rapid_enabled)
                .disabled(rapid_disabled)
                .on_change(cx.listener(|this, value, _, cx| this.toggle_rapid(*value, cx))),
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(rapid_help_control(
                    "keyboard-rapid-trigger-help",
                    t("RAPID_TRIGGER_TOOLTIP_V2").into(),
                )),
            cx,
        )
        .relative()
        .child(div().child(t("RAPID_TRIGGER_TITLE_DESC_V2")))
        .child(
            v_flex()
                .opacity(if inactive { 0.3 } else { 1. })
                .child(
                    div()
                        .pt(surface::css(16.))
                        .pb(surface::css(10.))
                        .child(t("RAPID_TRIGGER_SENSITIVITY_TITLE")),
                )
                .child(
                    div()
                        .pb(surface::css(10.))
                        .text_color(cx.theme().muted_foreground)
                        .child(t("CONFIGURE_DESC")),
                )
                .child(div().mt(surface::css(10.)).child(format!(
                    "{:.1} mm",
                    state.rapid_make.read(cx).value().start()
                )))
                .child(Slider::new(&state.rapid_make).disabled(inactive))
                .child(endpoints()),
        );
        if analog_v2 && state.rapid_make.read(cx).value().start() > 0.7 {
            rapid = rapid.child(
                div()
                    .mt(surface::css(20.))
                    .text_color(razer_widgets::theme::KeyboardActuationColors::warning())
                    .child(t("RAPID_TRIGGER_WARNING")),
            );
        }
        if state.rapid_upstroke {
            rapid = rapid.child(
                v_flex()
                    .opacity(if inactive { 0.3 } else { 1. })
                    .child(
                        div()
                            .pt(surface::css(16.))
                            .child(t("RAPID_TRIGGER_UPSTROKE_TEXT")),
                    )
                    .child(div().mt(surface::css(10.)).child(format!(
                        "{:.1} mm",
                        state.rapid_break.read(cx).value().start()
                    )))
                    .child(Slider::new(&state.rapid_break).disabled(inactive))
                    .child(endpoints()),
            );
        }
        rapid = rapid
            .child(
                Checkbox::new("keyboard-rapid-upstroke")
                    .label(t("RAPID_TRIGGER_UPSTROKE_CHECKBOX"))
                    .checked(state.rapid_upstroke)
                    .disabled(inactive)
                    .on_click(
                        cx.listener(|this, value, _, cx| this.toggle_rapid_upstroke(*value, cx)),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .child(sync_button(
                        "rapid-sync-all",
                        "SYNC_SETTINGS_TO_ALL_KEYS",
                        inactive,
                        true,
                        true,
                        cx,
                    ))
                    .child(sync_button(
                        "rapid-sync-selected",
                        "SYNC_SETTINGS_TO_SELECTED_KEYS",
                        inactive,
                        false,
                        true,
                        cx,
                    )),
            );
        let right =
            v_flex()
                .child(rapid)
                .when(self.continuous_supported(), |column| {
                    column.child(
                        surface::panel_with_title_switch(
                            t("CONTINUOUS_RAPID_TRIGGER"),
                            surface::SynapseSwitch::new("keyboard-continuous-rapid-trigger")
                                .checked(state.continuous_enabled)
                                .disabled(locked)
                                .on_change(cx.listener(|this, value, _, cx| {
                                    this.toggle_continuous(*value, cx)
                                })),
                            div()
                                .absolute()
                                .right(surface::css(10.))
                                .top(surface::css(10.))
                                .child(surface::help_control(
                                    "keyboard-continuous-rapid-help",
                                    t("CONTINUOUS_RAPID_TRIGGER_TOOLTIP"),
                                )),
                            cx,
                        )
                        .relative()
                        .child(div().child(t("RAPID_TRIGGER_TOOLTIP"))),
                    )
                });
        v_flex()
            .gap_5()
            .when(locked, |page| {
                page.child(
                    surface::panel(t("FACTORY_DEFAULT_PROFILE_TITTLE"), cx)
                        .child(t("FACTORY_DEFAULT_PROFILE_WARNING_DESC")),
                )
            })
            .child(self.keyboard_image(true, cx))
            .child(keys)
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex().children(self.analog_snap_panel(cx)).child(panel),
                    ))
                    .child(surface::page_column(right)),
            )
            .into_any_element()
    }
}
