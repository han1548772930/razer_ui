//! Current 4264:se / 1958:N,S inspector. Working paint parameters are transient:
//! 9286:R / 1638:j merge them; only 9286:U applies them to device regions.
use super::studio_color::{StudioColor, StudioColorEvent};
use super::studio_color_dropdown::{ColorDropdownChanged, StudioColorDropdown};
use super::studio_duration::{DurationChanged, StudioDuration};
use super::studio_gradient::{GradientChanged, GradientKind, StudioGradient};
use super::studio_playback::{PlaybackChanged, StudioPlayback};
use super::*;
use crate::ui::source_tooltip::{SourceTooltip, SourceTooltipKind};
use gpui_kit::component::slider::SliderState;
#[path = "chroma_studio_numeric.rs"]
mod numeric;
use numeric::{NumericChanged, NumericField, StudioNumeric};

pub(super) struct StudioPropertiesChanged {
    pub(super) layer_id: u64,
    pub(super) params: Value,
    pub(super) params2: Value,
    pub(super) paint_params: Value,
}
impl EventEmitter<StudioPropertiesChanged> for StudioProperties {}

pub(super) struct StudioProperties {
    current: Option<(u64, String)>,
    tool: String,
    params: Value,
    params2: Value,
    layer_params: Value,
    paint_params: Value,
    blur: Entity<SliderState>,
    duration: Entity<StudioDuration>,
    color: Option<Entity<StudioColor>>,
    color_subscription: Option<Subscription>,
    gradient: Option<Entity<StudioGradient>>,
    gradient_subscription: Option<Subscription>,
    colors: Option<[Entity<StudioColorDropdown>; 2]>,
    playback: Option<Entity<StudioPlayback>>,
    numeric: Option<[Entity<StudioNumeric>; 11]>,
    control_revision: u64,
    editor_subscriptions: Vec<Subscription>,
    window: Option<AnyWindowHandle>,
    _subscriptions: Vec<Subscription>,
}

impl StudioProperties {
    pub(super) fn new(owner: &Entity<ChromaStudio>, cx: &mut Context<Self>) -> Self {
        let blur = cx.new(|_| Self::blur_state(5.));
        let duration = cx.new(StudioDuration::new);
        let subscriptions = vec![
            cx.subscribe(&duration, |this, _, event: &DurationChanged, cx| {
                if !this.enabled() {
                    return;
                }
                let value = this
                    .current
                    .as_ref()
                    .filter(|(_, name)| {
                        matches!(
                            name.as_str(),
                            "spectrum" | "breathing" | "reactive" | "starlight"
                        )
                    })
                    .and_then(|(_, name)| {
                        source().effects.iter().find(|effect| &effect.name == name)
                    })
                    .and_then(|effect| effect.duration_values.get(event.0))
                    .copied();
                if let Some(value) = value {
                    this.params["duration"] = Value::from(value);
                    this.publish(cx);
                    cx.notify();
                }
            }),
            cx.observe(owner, |this, owner, cx| {
                let owner = owner.read(cx);
                let layer = owner
                    .document
                    .layers
                    .iter()
                    .find(|layer| Some(layer.id) == owner.current && !layer.group)
                    .map(|layer| {
                        (
                            (layer.id, layer.name.clone()),
                            layer.params.clone(),
                            layer.params2.clone(),
                            layer.paint_params.clone(),
                        )
                    });
                let tool = owner.tool.clone();
                this.sync(layer, tool, cx);
            }),
            cx.observe(&blur, |this, slider, cx| {
                // Base's accessibility actions use set_value without emitting
                // SliderEvent. Observe the retained value for all input paths.
                let value = slider.read(cx).value().start().round();
                let expected = this.params["blur"].as_f64().unwrap_or(5.) as f32;
                if value == expected {
                    return;
                }
                if this.enabled()
                    && this
                        .current
                        .as_ref()
                        .is_some_and(|(_, name)| name == "ambient")
                {
                    this.params["blur"] = Value::from(value as u32);
                    this.publish(cx);
                    cx.notify();
                } else {
                    // The base accessibility callback is available even when
                    // disabled. A disabled source control cannot change state.
                    slider.update(cx, |slider, cx| {
                        *slider = Self::blur_state(expected);
                        cx.notify();
                    });
                }
            }),
        ];
        Self {
            current: None,
            tool: "select".into(),
            params: serde_json::json!({}),
            params2: serde_json::json!({}),
            layer_params: serde_json::json!({}),
            paint_params: serde_json::json!({}),
            blur,
            duration,
            color: None,
            color_subscription: None,
            gradient: None,
            gradient_subscription: None,
            colors: None,
            playback: None,
            numeric: None,
            control_revision: 0,
            editor_subscriptions: Vec::new(),
            window: None,
            _subscriptions: subscriptions,
        }
    }

    fn blur_state(value: f32) -> SliderState {
        SliderState::new()
            .min(1.)
            .max(9.)
            .step(1.)
            .default_value(value)
    }

    fn enabled(&self) -> bool {
        self.current.is_some() && matches!(self.tool.as_str(), "pen" | "bucket")
    }

    fn publish(&mut self, cx: &mut Context<Self>) {
        let Some((layer_id, _)) = self.current else {
            return;
        };
        if self.enabled() {
            self.paint_params = self.params.clone();
        } else {
            self.layer_params = self.params.clone();
        }
        cx.emit(StudioPropertiesChanged {
            layer_id,
            params: self.layer_params.clone(),
            params2: self.params2.clone(),
            paint_params: self.paint_params.clone(),
        });
    }

    fn color_value(&self) -> Option<u32> {
        self.params["color"]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
    }

    fn refresh_duration(&self, cx: &mut Context<Self>) {
        let name = self
            .current
            .as_ref()
            .map(|(_, name)| name.as_str())
            .unwrap_or("");
        let index = source()
            .effects
            .iter()
            .find(|effect| effect.name == name)
            .and_then(|effect| {
                effect
                    .duration_values
                    .iter()
                    .position(|value| Some(*value) == self.params["duration"].as_u64())
            })
            .unwrap_or(1);
        self.duration.update(cx, |duration, cx| {
            duration.configure(
                name,
                index,
                self.enabled(),
                matches!(name, "spectrum" | "breathing" | "reactive" | "starlight"),
                cx,
            );
        });
    }

    pub(super) fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.duration
            .update(cx, |duration, cx| duration.attach(window, cx));
        let handle = window.window_handle();
        if self
            .window
            .is_some_and(|old| old.window_id() == handle.window_id())
            && self.color.is_some()
        {
            return;
        }
        // Input subscriptions are created for the current host only. This
        // retained session may have outlived its previous Chroma host window.
        let color = cx.new(|cx| StudioColor::new(self.color_value(), window, cx));
        color.update(cx, |color, cx| {
            color.set_enabled(self.enabled(), window, cx)
        });
        self.color_subscription = Some(cx.subscribe(&color, |this, _, event, cx| {
            if !this.enabled()
                || !this
                    .current
                    .as_ref()
                    .is_some_and(|(_, name)| name == "static")
            {
                return;
            }
            let StudioColorEvent::Changed(value) = event;
            if this.color_value() != *value {
                if let Some(value) = value {
                    this.params["color"] = Value::from(*value);
                } else if let Some(params) = this.params.as_object_mut() {
                    params.remove("color");
                }
                this.publish(cx);
                cx.notify();
            }
        }));
        self.color = Some(color);
        let cache = self
            .gradient
            .as_ref()
            .and_then(|gradient| gradient.read(cx).custom_cache());
        let gradient = cx.new(|cx| StudioGradient::new(window, cx));
        gradient.update(cx, |gradient, cx| {
            gradient.restore_cache(cache);
            gradient.configure(
                self.current
                    .as_ref()
                    .map(|(_, name)| name.as_str())
                    .unwrap_or(""),
                self.gradient_kind(),
                &self.params["colorStops"],
                self.params["colorStopsCustom"] == true,
                self.gradient_enabled(),
                self.gradient_hidden(),
                true,
                window,
                cx,
            )
        });
        self.gradient_subscription = Some(cx.subscribe(
            &gradient,
            |this, _, event: &GradientChanged, cx| {
                if this.gradient_enabled() {
                    this.params["colorStops"] = event.stops.clone();
                    this.params["colorStopsCustom"] = event.custom.into();
                    this.publish(cx);
                    cx.notify();
                }
            },
        ));
        self.gradient = Some(gradient);
        self.editor_subscriptions.clear();
        let colors =
            [0., 63.].map(|shift| cx.new(|cx| StudioColorDropdown::new(shift, window, cx)));
        for (index, color) in colors.iter().enumerate() {
            self.editor_subscriptions.push(cx.subscribe(
                color,
                move |this, _, event: &ColorDropdownChanged, cx| {
                    let name = this
                        .current
                        .as_ref()
                        .map(|(_, name)| name.as_str())
                        .unwrap_or("");
                    if !this.enabled()
                        || !matches!(name, "breathing" | "fire" | "reactive" | "tidal")
                        || (name == "reactive" && index != 0)
                        || (matches!(name, "breathing" | "reactive" | "tidal")
                            && this.params["randomColor"] == true)
                    {
                        return;
                    }
                    let field = if index == 0 { "color" } else { "color2" };
                    // 2777 intentionally maps black (0) as well as undefined to FU.
                    let value = if matches!(name, "breathing" | "tidal") {
                        Some(
                            event
                                .0
                                .filter(|value| *value != 0)
                                .unwrap_or(source().empty_color),
                        )
                    } else {
                        event.0
                    };
                    if let Some(value) = value {
                        this.params[field] = value.into();
                    } else if let Some(params) = this.params.as_object_mut() {
                        params.remove(field);
                    }
                    this.refresh_pairs(true, cx);
                    this.publish(cx);
                    cx.notify();
                },
            ));
        }
        self.colors = Some(colors);
        let playback = cx.new(|cx| StudioPlayback::new(window, cx));
        self.editor_subscriptions.push(cx.subscribe(
            &playback,
            |this, _, event: &PlaybackChanged, cx| {
                if this.enabled()
                    && this.current.as_ref().is_some_and(|(_, name)| {
                        matches!(
                            name.as_str(),
                            "breathing" | "ripple" | "wave" | "wheel" | "tidal"
                        )
                    })
                {
                    if let (Some(params), Some(patch)) =
                        (this.params.as_object_mut(), event.0.as_object())
                    {
                        params.extend(patch.clone());
                    }
                    this.publish(cx);
                    cx.notify();
                }
            },
        ));
        self.playback = Some(playback);
        let numeric = [
            NumericField::RippleSpeed,
            NumericField::RippleWidth,
            NumericField::StarlightDensity,
            NumericField::WaveSpeed,
            NumericField::WaveWidth,
            NumericField::WavePause,
            NumericField::WaveAngle,
            NumericField::WheelSpeed,
            NumericField::TidalSpeed,
            NumericField::AudioBoost,
            NumericField::AudioDecay,
        ]
        .map(|field| cx.new(|cx| StudioNumeric::new(field, window, cx)));
        for editor in &numeric {
            self.editor_subscriptions.push(cx.subscribe(
                editor,
                |this, _, event: &NumericChanged, cx| {
                    if this.enabled()
                        && event.revision == this.control_revision
                        && this
                            .current
                            .as_ref()
                            .is_some_and(|(_, name)| name == event.field.effect())
                    {
                        let value = Value::from(event.value);
                        if this.params[event.field.field()] != value {
                            this.params[event.field.field()] = value;
                            this.publish(cx);
                            cx.notify();
                        }
                    }
                },
            ));
        }
        self.numeric = Some(numeric);
        self.window = Some(handle);
        self.refresh_pairs(true, cx);
        self.refresh_playback(cx);
        self.refresh_numeric(cx);
        cx.notify();
    }

    fn refresh_pairs(&self, replace: bool, cx: &mut Context<Self>) {
        let (Some(colors), Some(handle)) = (self.colors.clone(), self.window) else {
            return;
        };
        let name = self
            .current
            .as_ref()
            .map(|(_, name)| name.as_str())
            .unwrap_or("");
        let hidden = matches!(name, "breathing" | "reactive" | "tidal")
            && self.params["randomColor"] == true;
        let enabled = self.enabled()
            && matches!(name, "breathing" | "fire" | "reactive" | "tidal")
            && !hidden;
        let reactive = name == "reactive";
        let effect = name.to_owned();
        let values = ["color", "color2"].map(|field| {
            self.params[field]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| reactive || *value != source().empty_color)
        });
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                for (index, (color, value)) in colors.iter().zip(values).enumerate() {
                    color.update(cx, |color, cx| {
                        color.configure(
                            &effect,
                            value,
                            enabled && (!reactive || index == 0),
                            hidden,
                            replace,
                            window,
                            cx,
                        )
                    });
                }
            });
        });
    }

    fn refresh_playback(&self, cx: &mut Context<Self>) {
        let (Some(playback), Some(handle)) = (self.playback.clone(), self.window) else {
            return;
        };
        let params = self.params.clone();
        let effect = self
            .current
            .as_ref()
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let enabled = self.enabled();
        let mounted = self.current.as_ref().is_some_and(|(_, name)| {
            matches!(
                name.as_str(),
                "breathing" | "ripple" | "wave" | "wheel" | "tidal"
            )
        });
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                playback.update(cx, |playback, cx| {
                    playback.configure(&effect, &params, enabled, mounted, window, cx)
                })
            });
        });
    }

    fn refresh_color(&self, replace_value: bool, cx: &mut Context<Self>) {
        self.refresh_pairs(replace_value, cx);
        self.refresh_playback(cx);
        if let (Some(gradient), Some(handle)) = (self.gradient.clone(), self.window) {
            let value = self.params["colorStops"].clone();
            let custom = self.params["colorStopsCustom"] == true;
            let enabled = self.gradient_enabled();
            let hidden = self.gradient_hidden();
            let kind = self.gradient_kind();
            let effect = self
                .current
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_default();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    gradient.update(cx, |gradient, cx| {
                        gradient.configure(
                            &effect,
                            kind,
                            &value,
                            custom,
                            enabled,
                            hidden,
                            replace_value,
                            window,
                            cx,
                        );
                    })
                });
            });
        }
        let (Some(color), Some(handle)) = (self.color.clone(), self.window) else {
            return;
        };
        let value = self.color_value();
        let enabled = self.enabled();
        // Plain parent observation is not tied to a stale window. Defer the
        // window-bound input synchronization out of the properties update.
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                color.update(cx, |color, cx| {
                    if replace_value {
                        color.set_value(value, window, cx);
                    }
                    color.set_enabled(enabled, window, cx);
                })
            });
        });
    }

    fn gradient_kind(&self) -> GradientKind {
        if self
            .current
            .as_ref()
            .is_some_and(|(_, name)| name == "spectrum")
        {
            GradientKind::Spectrum
        } else if self
            .current
            .as_ref()
            .is_some_and(|(_, name)| name == "audio")
        {
            GradientKind::Audio
        } else {
            GradientKind::Default
        }
    }

    fn gradient_hidden(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|(_, name)| name == "starlight")
            && self.params["randomColor"] == true
    }

    fn gradient_enabled(&self) -> bool {
        self.enabled()
            && !self.gradient_hidden()
            && self.current.as_ref().is_some_and(|(_, name)| {
                matches!(
                    name.as_str(),
                    "spectrum" | "ripple" | "starlight" | "audio" | "wave" | "wheel"
                )
            })
    }

    fn refresh_numeric(&self, cx: &mut Context<Self>) {
        let (Some(editors), Some(handle)) = (self.numeric.clone(), self.window) else {
            return;
        };
        let params = self.params.clone();
        let effect = self
            .current
            .as_ref()
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let enabled = self.enabled();
        let revision = self.control_revision;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                for (editor, field) in editors.iter().zip([
                    NumericField::RippleSpeed,
                    NumericField::RippleWidth,
                    NumericField::StarlightDensity,
                    NumericField::WaveSpeed,
                    NumericField::WaveWidth,
                    NumericField::WavePause,
                    NumericField::WaveAngle,
                    NumericField::WheelSpeed,
                    NumericField::TidalSpeed,
                    NumericField::AudioBoost,
                    NumericField::AudioDecay,
                ]) {
                    editor.update(cx, |editor, cx| {
                        let active = enabled
                            && effect == field.effect()
                            && !(field == NumericField::AudioBoost && params["autoBoost"] == true);
                        editor.configure(
                            params[field.field()].as_f64().unwrap_or(0.),
                            active,
                            revision,
                            window,
                            cx,
                        )
                    });
                }
            });
        });
    }

    pub(super) fn reset(&mut self, cx: &mut Context<Self>) {
        if !self.enabled() {
            return;
        }
        if let Some(effect) = self
            .current
            .as_ref()
            .and_then(|(_, name)| source().effects.iter().find(|effect| &effect.name == name))
        {
            self.control_revision = self.control_revision.wrapping_add(1);
            if let (Some(params), Some(defaults)) =
                (self.params.as_object_mut(), effect.paint_params.as_object())
            {
                params.extend(defaults.clone());
            }
            let blur = self.params["blur"].as_f64().unwrap_or(5.) as f32;
            self.blur.update(cx, |slider, cx| {
                *slider = Self::blur_state(blur);
                cx.notify();
            });
            self.refresh_color(true, cx);
            self.refresh_duration(cx);
            self.refresh_numeric(cx);
            self.publish(cx);
            cx.notify();
        }
    }

    fn sync(
        &mut self,
        layer: Option<((u64, String), Value, Value, Value)>,
        tool: String,
        cx: &mut Context<Self>,
    ) {
        let (current, params, params2, paint_params) = layer
            .map(|(current, params, params2, paint_params)| {
                (Some(current), params, params2, paint_params)
            })
            .unwrap_or_else(|| {
                (
                    None,
                    serde_json::json!({}),
                    serde_json::json!({}),
                    serde_json::json!({}),
                )
            });
        let selection_changed = current != self.current;
        let tool_changed = tool != self.tool;
        if !selection_changed && !tool_changed {
            return;
        }
        // 9870:d keeps pen↔bucket values; transitions involving select/move
        // merge defaults (1638:j zB), whereas 9286:C replaces on layer change.
        self.current = current;
        self.tool = tool;
        self.layer_params = params;
        self.params2 = params2;
        self.paint_params = paint_params;
        self.params = if self.enabled() {
            self.paint_params.clone()
        } else {
            self.layer_params.clone()
        };
        self.control_revision = self.control_revision.wrapping_add(1);
        let blur = self.params["blur"].as_f64().unwrap_or(5.) as f32;
        self.blur.update(cx, |slider, cx| {
            *slider = Self::blur_state(blur);
            cx.notify();
        });
        self.refresh_color(true, cx);
        self.refresh_duration(cx);
        self.refresh_numeric(cx);
        cx.notify();
    }

    fn subtitle(&self, title: &'static str, help: &'static str) -> AnyElement {
        let tip = SourceTooltip::new(
            (ElementId::from("studio-property-help"), title),
            label(help),
            300.,
        )
        .kind(SourceTooltipKind::Studio)
        .trigger(move |_, _, _| {
            button(
                (ElementId::from("studio-property-help-button"), title),
                help,
            )
            .size(surface::css(14.))
            .rounded_full()
            .bg(Colors::help_background())
            .child(icon("tooltip_questionmark", 14.))
            .into_any_element()
        });
        div()
            .flex()
            .items_center()
            .min_h(surface::css(34.))
            .px(surface::css(10.))
            .py(surface::css(8.))
            .child(div().flex_1().child(label(title).to_uppercase()))
            .child(if self.enabled() {
                tip.into_any_element()
            } else {
                div()
                    .size(surface::css(14.))
                    .rounded_full()
                    .bg(Colors::help_background())
                    .child(icon("tooltip_questionmark", 14.))
                    .into_any_element()
            })
            .into_any_element()
    }

    fn direction(&self, id: &'static str, tidal: bool, cx: &mut Context<Self>) -> AnyElement {
        let counterclockwise = self.params2["counterclockwise"] == true;
        let enabled = self.enabled();
        let owner = cx.weak_entity();
        let clockwise_label = if tidal { "Outward" } else { "Clockwise" };
        let counterclockwise_label = if tidal { "Inward" } else { "Counterclockwise" };
        let option = |suffix: &'static str, text: &'static str, selected: bool, value: bool| {
            let owner = owner.clone();
            button((ElementId::from(id), suffix), "TEXT_DIRECTION")
                .accessibility_label(text)
                .disabled(!enabled)
                .min_w(surface::css(52.))
                .h(surface::css(40.))
                .px(surface::css(8.))
                .border_1()
                .border_color(if selected {
                    Colors::selected()
                } else {
                    Colors::border()
                })
                .bg(if selected {
                    Colors::selected()
                } else {
                    Colors::panel()
                })
                .child(text)
                .on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.enabled()
                            && this.params2["counterclockwise"] != Value::from(value)
                            && this.current.as_ref().is_some_and(|(_, name)| {
                                matches!(name.as_str(), "wheel" | "tidal")
                                    && (tidal == (name == "tidal"))
                            })
                        {
                            this.params2["counterclockwise"] = Value::from(value);
                            this.publish(cx);
                            cx.notify();
                        }
                    });
                })
        };
        div()
            .id((ElementId::from(id), "direction"))
            .test_support()
            .flex()
            .items_center()
            .gap(surface::css(6.))
            .child(option(
                "clockwise",
                clockwise_label,
                !counterclockwise,
                false,
            ))
            .child(option(
                "counterclockwise",
                counterclockwise_label,
                counterclockwise,
                true,
            ))
            .into_any_element()
    }

    fn region_box(region: &str, preset: bool) -> Div {
        let thickness = surface::css(if preset { 4. } else { 30. });
        let rect = div().absolute().bg(if preset {
            Colors::white()
        } else {
            Colors::text()
        });
        match region {
            "full" => rect.inset_0(),
            "left" => rect.left_0().top_0().h_full().w(thickness),
            "right" => rect.right_0().top_0().h_full().w(thickness),
            "top" => rect.left_0().top_0().w_full().h(thickness),
            "bottom" => rect.left_0().bottom_0().w_full().h(thickness),
            _ => rect.size_0(),
        }
    }

    fn paired_colors(
        &self,
        breathing: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut colors = div().flex().gap(surface::css(10.));
        if let Some(pair) = &self.colors {
            for (index, color) in pair.iter().enumerate() {
                colors = colors.child(
                    div()
                        .flex()
                        .flex_col()
                        .when(!breathing, |view| {
                            view.child(div().mb(surface::css(6.)).child(label(if index == 0 {
                                "TEXT_HOT"
                            } else {
                                "TEXT_COLD"
                            })))
                        })
                        .child(color.clone()),
                );
            }
        }
        let mut row = div()
            .flex()
            .child(colors.when(breathing, |view| view.mr(surface::css(10.))));
        if breathing {
            let owner = cx.weak_entity();
            row = row.child(
                super::studio_checkbox::checkbox(
                    "studio-breathing-random",
                    self.params["randomColor"] == true,
                    self.enabled(),
                    label("RANDOM"),
                    window,
                    cx,
                )
                .on_change(move |state, _, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.enabled() {
                            this.params["randomColor"] =
                                (state == gpui_kit::base::CheckboxState::Checked).into();
                            this.refresh_pairs(false, cx);
                            this.publish(cx);
                            cx.notify();
                        }
                    });
                }),
            );
        }
        div()
            .flex()
            .flex_col()
            .child(self.subtitle(
                "COLOR",
                if breathing {
                    "TEXT_BREATHING_COLOR"
                } else {
                    "TEXT_FIRE_COLOR"
                },
            ))
            .child(section().child(row))
            .when(breathing, |view| {
                view.child(self.subtitle("PROPERTIES", "TEXT_BREATHING_PROPERTIES"))
                    .child(section().child(self.duration.clone()))
                    .child(self.subtitle("PLAYBACK_TITLE", "TEXT_PLAYBACK"))
                    .child(section().children(self.playback.clone()))
            })
            .into_any_element()
    }

    fn reactive(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let revision = self.control_revision;
        let random = super::studio_checkbox::checkbox(
            "studio-reactive-random",
            self.params["randomColor"] == true,
            self.enabled(),
            label("RANDOM"),
            window,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.enabled()
                    && this.control_revision == revision
                    && this
                        .current
                        .as_ref()
                        .is_some_and(|(_, name)| name == "reactive")
                {
                    this.params["randomColor"] =
                        (state == gpui_kit::base::CheckboxState::Checked).into();
                    this.refresh_pairs(false, cx);
                    this.publish(cx);
                    cx.notify();
                }
            });
        });
        div()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_REACTIVE_COLOR"))
            .child(
                section().child(
                    div()
                        .flex()
                        .items_center()
                        .gap(surface::css(10.))
                        .children(self.colors.as_ref().map(|colors| colors[0].clone()))
                        .child(random),
                ),
            )
            .child(self.subtitle("PROPERTIES", "TEXT_REACTIVE_PROPERTIES"))
            .child(section().child(self.duration.clone()))
            .into_any_element()
    }

    fn ripple(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_RIPPLE_COLOR"))
            .child(section().children(self.gradient.clone()))
            .child(self.subtitle("PROPERTIES", "TEXT_RIPPLE_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[0].clone()))
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .children(self.numeric.as_ref().map(|editors| editors[1].clone())),
                    ),
            )
            .child(self.subtitle("PLAYBACK_TITLE", "TEXT_PLAYBACK"))
            .child(section().children(self.playback.clone()))
            .into_any_element()
    }

    fn wave(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let split = super::studio_checkbox::checkbox(
            "studio-wave-split",
            self.params["split"] == true,
            self.enabled(),
            label("TEXT_SPLIT"),
            window,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.enabled() {
                    this.params["split"] = (state == gpui_kit::base::CheckboxState::Checked).into();
                    this.publish(cx);
                    cx.notify();
                }
            });
        });
        div()
            .id("studio-wave-properties")
            .test_support()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_WAVE_COLOR"))
            .child(section().children(self.gradient.clone()))
            .child(self.subtitle("PROPERTIES", "TEXT_WAVE_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[3].clone()))
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .children(self.numeric.as_ref().map(|editors| editors[4].clone())),
                    )
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .children(self.numeric.as_ref().map(|editors| editors[5].clone())),
                    )
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .children(self.numeric.as_ref().map(|editors| editors[6].clone())),
                    )
                    .child(div().mt(surface::css(10.)).child(split)),
            )
            .child(self.subtitle("PLAYBACK_TITLE", "TEXT_PLAYBACK"))
            .child(section().children(self.playback.clone()))
            .into_any_element()
    }

    fn wheel(&self, cx: &mut Context<Self>) -> AnyElement {
        let direction = self.direction("studio-wheel-direction", false, cx);
        div()
            .id("studio-wheel-properties")
            .test_support()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_WHEEL_COLOR"))
            .child(section().children(self.gradient.clone()))
            .child(self.subtitle("PROPERTIES", "TEXT_WHEEL_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[7].clone()))
                    .child(div().mt(surface::css(10.)).child(direction)),
            )
            .child(self.subtitle("PLAYBACK_TITLE", "TEXT_PLAYBACK"))
            .child(section().children(self.playback.clone()))
            .into_any_element()
    }

    fn tidal(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let random = super::studio_checkbox::checkbox(
            "studio-tidal-random",
            self.params["randomColor"] == true,
            self.enabled(),
            label("RANDOM"),
            window,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.enabled() {
                    this.params["randomColor"] =
                        (state == gpui_kit::base::CheckboxState::Checked).into();
                    this.refresh_pairs(false, cx);
                    this.publish(cx);
                    cx.notify();
                }
            });
        });
        let direction = self.direction("studio-tidal-direction", true, cx);
        div()
            .id("studio-tidal-properties")
            .test_support()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_WAVE_COLOR"))
            .child(section().child({
                let colors = self.colors.as_ref();
                let mut row = div().flex().gap(surface::css(10.));
                if let Some(colors) = colors {
                    row = row.child(colors[0].clone()).child(colors[1].clone());
                }
                row.child(random)
            }))
            .child(self.subtitle("PROPERTIES", "TEXT_WHEEL_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[8].clone()))
                    .child(div().mt(surface::css(10.)).child(direction)),
            )
            .child(self.subtitle("PLAYBACK_TITLE", "TEXT_PLAYBACK"))
            .child(section().children(self.playback.clone()))
            .into_any_element()
    }

    fn audio(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let auto = super::studio_checkbox::checkbox(
            "studio-audio-auto-boost",
            self.params["autoBoost"] == true,
            self.enabled(),
            label("TEXT_BOOST"),
            window,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.enabled() {
                    this.params["autoBoost"] =
                        (state == gpui_kit::base::CheckboxState::Checked).into();
                    this.refresh_numeric(cx);
                    this.publish(cx);
                    cx.notify();
                }
            });
        });
        div()
            .id("studio-audio-properties")
            .test_support()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_AUDIO_METER_COLOR"))
            .child(section().children(self.gradient.clone()))
            .child(self.subtitle("PROPERTIES", "TEXT_AUDIO_METER_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[9].clone()))
                    .child(div().mt(surface::css(10.)).child(auto))
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .children(self.numeric.as_ref().map(|editors| editors[10].clone())),
                    ),
            )
            .into_any_element()
    }

    fn starlight(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let revision = self.control_revision;
        let random = super::studio_checkbox::checkbox(
            "studio-starlight-random",
            self.params["randomColor"] == true,
            self.enabled(),
            label("RANDOM"),
            window,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.enabled()
                    && this.control_revision == revision
                    && this
                        .current
                        .as_ref()
                        .is_some_and(|(_, name)| name == "starlight")
                {
                    this.params["randomColor"] =
                        (state == gpui_kit::base::CheckboxState::Checked).into();
                    this.refresh_color(false, cx);
                    this.publish(cx);
                    cx.notify();
                }
            });
        });
        div()
            .flex()
            .flex_col()
            .child(self.subtitle("COLOR", "TEXT_STARLIGHT_COLOR"))
            .child(section().children(self.gradient.clone()).child(random))
            .child(self.subtitle("PROPERTIES", "TEXT_STARLIGHT_PROPERTIES"))
            .child(
                section()
                    .children(self.numeric.as_ref().map(|editors| editors[2].clone()))
                    .child(div().mt(surface::css(10.)).child(self.duration.clone())),
            )
            .into_any_element()
    }

    fn ambient(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.enabled();
        let region = self.params["screen"].as_str().unwrap_or("");
        let mut presets = div().flex().flex_wrap();
        for name in ["full", "left", "top", "right", "bottom"] {
            presets = presets.child(super::studio_region_preset::RegionPreset::new(
                name,
                region == name,
                enabled,
                cx.listener(move |this, _, _, cx| {
                    if this.enabled() {
                        this.params["screen"] = Value::from(name);
                        this.publish(cx);
                        cx.notify();
                    }
                }),
            ));
        }
        div()
            .flex()
            .flex_col()
            .child(self.subtitle("TEXT_REGION", "TEXT_AMBIENT_AWARENESS_REGION"))
            .child(
                section()
                    .child(
                        div()
                            .relative()
                            .h(surface::css(128.))
                            .mb(surface::css(10.))
                            .border_1()
                            .border_color(Colors::helper())
                            .bg(Colors::region_background())
                            .child(
                                div()
                                    .relative()
                                    .size_full()
                                    .child(Self::region_box(region, false)),
                            ),
                    )
                    // 1958:N starts the native region service. No device/native
                    // bridge exists locally, so this command cannot invent a region.
                    .child(
                        button("studio-edit-region", "TEXT_EDIT_REGION")
                            .disabled(true)
                            .styles(|style| {
                                style
                                    .disabled(|style| style.opacity(if enabled { 0.3 } else { 1. }))
                            })
                            .hover(|style| style.bg(Colors::panel()))
                            .w_full()
                            .h(surface::css(27.))
                            .mb(surface::css(10.))
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(Colors::text())
                            .text_size(surface::css(12.))
                            .child(label("TEXT_EDIT_REGION").to_uppercase()),
                    )
                    .child(div().mb(surface::css(6.)).child(label("TEXT_PRESETS")))
                    .child(presets),
            )
            .child(self.subtitle("PROPERTIES", "TEXT_AMBIENT_AWARENESS_PROPERTIES"))
            .child(
                section()
                    .child(div().mb(surface::css(6.)).child(label("TEXT_BLEND")))
                    .child(
                        super::studio_slider::StudioSlider::new(&self.blur, enabled)
                            .label(label("TEXT_BLEND"))
                            .on_key_change({
                                let owner = cx.entity();
                                move |value, _, cx| {
                                    owner.update(cx, |this, cx| {
                                        if this.enabled() {
                                            this.params["blur"] = Value::from(value.round() as u32);
                                            this.publish(cx);
                                            cx.notify();
                                        }
                                    })
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .mt(surface::css(3.))
                            .mb(surface::css(10.))
                            .child(label("TEXT_SHARP"))
                            .child(label("TEXT_BLUR")),
                    )
                    .child(
                        div()
                            .text_color(Colors::helper())
                            .child(label("TEXT_AMBIENT_SLIDER_HELPER")),
                    ),
            )
            .into_any_element()
    }
}

fn section() -> Div {
    div()
        .flex()
        .flex_col()
        .px(surface::css(10.))
        .pb(surface::css(20.))
        .border_b_1()
        .border_color(Colors::border())
}

impl Render for StudioProperties {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opacity = surface::fade_opacity(
            "studio-properties-opacity",
            if self.enabled() { 1. } else { 0.3 },
            100,
            window,
            cx,
        );
        let content = match self.current.as_ref().map(|(_, name)| name.as_str()) {
            Some("breathing") => self.paired_colors(true, window, cx),
            Some("fire") => self.paired_colors(false, window, cx),
            Some("reactive") => self.reactive(window, cx),
            Some("ripple") => self.ripple(),
            Some("wave") => self.wave(window, cx),
            Some("wheel") => self.wheel(cx),
            Some("tidal") => self.tidal(window, cx),
            Some("audio") => self.audio(window, cx),
            Some("starlight") => self.starlight(window, cx),
            Some("ambient") => self.ambient(cx),
            Some("static") => div()
                .flex()
                .flex_col()
                .child(self.subtitle("COLOR", "TEXT_STATIC_COLOR"))
                .child(section().children(self.color.clone()))
                .into_any_element(),
            Some("spectrum") => div()
                .flex()
                .flex_col()
                .child(self.subtitle("COLOR", "TEXT_SPECTRUM_CYCLING_COLOR"))
                .child(section().children(self.gradient.clone()))
                .child(self.subtitle("PROPERTIES", "TEXT_SPECTRUM_CYCLING_PROPERTIES"))
                .child(section().child(self.duration.clone()))
                .into_any_element(),
            _ => div().into_any_element(),
        };
        div()
            .id("studio-properties")
            .test_support()
            .line_height(relative(1.22))
            .flex_1()
            .min_h_0()
            .scrollable_y()
            .border_t_1()
            .border_color(Colors::border())
            .child(div().opacity(opacity).child(content))
    }
}
