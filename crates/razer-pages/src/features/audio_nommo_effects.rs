//! Current 1303 `bm/Kp` and 1304 `wM/qm`. See nommo-effects-source.json.
//! State here is a local draft; no connected device or Chroma service is invented.
use super::*;
use crate::features::lighting_color::LightingColorPicker;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, Tabs};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use razer_widgets::stepper::Stepper;
use razer_widgets::stepper::StepperEvent;
use std::time::Duration;
#[path = "audio_nommo_effects_theme.rs"]
mod theme;
use theme::{Colors, CssColor};

#[derive(Deserialize)]
struct Effect {
    id: u32,
    name: String,
}
#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    effects: Vec<Effect>,
    wave_default_direction: i64,
    defaults: BTreeMap<String, Value>,
}
fn spec(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_nommo_effects_data.json"))
                .expect("validated current Nommo effects")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
}
fn choices(pid: u32) -> Vec<Choice> {
    spec(pid)
        .map(|s| {
            s.effects
                .iter()
                .map(|effect| Choice::new(effect.id.to_string(), t(&effect.name)))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) struct NommoEffectsState {
    effect: Entity<SelectState<Vec<Choice>>>,
    colors: [Entity<ColorPickerState>; 2],
    boost: Entity<Stepper>,
    chroma_profiles: Entity<SelectState<Vec<Choice>>>,
}
impl NommoEffectsState {
    pub(super) fn new(
        pid: u32,
        window: &mut Window,
        cx: &mut Context<AudioProductWorkspace>,
    ) -> Option<Self> {
        spec(pid)?;
        Some(Self {
            effect: cx.new(|cx| SelectState::new(choices(pid), None, window, cx)),
            colors: std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx))),
            boost: cx.new(|cx| {
                Stepper::new(
                    "nommo-color-boost",
                    1.,
                    (0.25, 4., 0.25),
                    true,
                    true,
                    Some(4),
                    window,
                    cx,
                )
                .in_modes_area()
                .reveal_spinners_on_hover()
            }),
            chroma_profiles: cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx)),
        })
    }
}

impl AudioProductWorkspace {
    pub(super) fn initialize_nommo_draft(&mut self) {
        let Some(spec) = spec(self.spec.product_id) else {
            return;
        };
        let mut settings = spec.defaults.clone();
        // Kp/qm.handleDefaultWaveDirection uses its direction map, rather than
        // the legacy profile's wave field or the global default direction 2.
        settings.insert("4".into(), json!({"direction":spec.wave_default_direction}));
        self.draft["nommoEffects"] = json!({"advanced":false,"settings":settings});
    }
    fn nommo_effect_path(&self) -> &'static str {
        if self.spec.product_id == 1303 {
            "/profile/quickEffects/effect"
        } else {
            "/profile/quickEffects/selectedEffectId"
        }
    }
    fn nommo_selected_effect(&self) -> u32 {
        self.draft
            .pointer(self.nommo_effect_path())
            .and_then(Value::as_u64)
            .unwrap_or(3) as u32
    }
    fn nommo_setting(&self) -> &Value {
        &self.draft["nommoEffects"]["settings"][self.nommo_selected_effect().to_string()]
    }
    fn nommo_advanced(&self) -> bool {
        self.draft["nommoEffects"]["advanced"] == true
    }
    pub(super) fn normalize_nommo_draft(&mut self) {
        let Some(spec) = spec(self.spec.product_id) else {
            return;
        };
        if !spec
            .effects
            .iter()
            .any(|e| e.id == self.nommo_selected_effect())
        {
            let path = self.nommo_effect_path();
            *self.draft.pointer_mut(path).expect("audited effect field") = json!(3);
        }
        let settings = &mut self.draft["nommoEffects"]["settings"];
        for id in [1, 2, 19] {
            for field in ["color1", "color2"] {
                let Some(default) = spec.defaults[&id.to_string()].get(field) else {
                    continue;
                };
                let value = &mut settings[id.to_string()][field];
                let valid = value.as_str().is_some_and(|value| {
                    value == "no-color" && id != 1
                        || value
                            .strip_prefix('#')
                            .is_some_and(|v| v.len() == 6 && u32::from_str_radix(v, 16).is_ok())
                });
                if !valid {
                    *value = default.clone();
                }
            }
        }
        for (id, valid, default) in [(4, [11, 12], spec.wave_default_direction), (19, [1, 0], 1)] {
            let value = &mut settings[id.to_string()]["direction"];
            if !value.as_i64().is_some_and(|v| valid.contains(&v)) {
                *value = json!(default);
            }
        }
        let boost = settings["12"]["colorBoost"].as_f64().unwrap_or(1.);
        settings["12"]["colorBoost"] = json!((boost.clamp(0.25, 4.) * 4.).ceil() / 4.);
        self.draft["profile"]["quickEffects"]["selectedEffectSetting"] =
            self.nommo_setting().clone();
    }
    fn change_nommo_setting(
        &mut self,
        field: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || self.nommo_advanced() {
            return;
        }
        let id = self.nommo_selected_effect().to_string();
        self.draft["nommoEffects"]["settings"][&id][field] = value;
        let setting = self.draft["nommoEffects"]["settings"][&id].clone();
        // Source setSelectedEffect keeps selectedEffectId/selectedEffectSetting together.
        self.draft["profile"]["quickEffects"]["selectedEffectSetting"] = setting;
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    pub(super) fn subscribe_nommo_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.nommo_effects else {
            return;
        };
        let effect = state.effect.clone();
        let colors = state.colors.clone();
        let boost = state.boost.clone();
        self.subscriptions.push(
            cx.subscribe_in(&effect, window, |this, _, event, window, cx| {
                if this.syncing || this.nommo_advanced() {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Some(id) = value.parse::<u32>().ok().filter(|id| {
                        spec(this.spec.product_id)
                            .is_some_and(|s| s.effects.iter().any(|e| e.id == *id))
                    }) {
                        let path = this.nommo_effect_path();
                        *this.draft.pointer_mut(path).expect("audited effect field") = json!(id);
                        this.draft["profile"]["quickEffects"]["selectedEffectSetting"] =
                            this.nommo_setting().clone();
                        this.sync(window, cx);
                        cx.emit(AudioProductChanged);
                        cx.notify();
                    }
                }
            }),
        );
        for (channel, color) in colors.iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                color,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing
                        || !matches!(this.nommo_selected_effect(), 1 | 2 | 19)
                        || this.nommo_setting()["isRandom"] == true
                    {
                        return;
                    }
                    let ColorPickerEvent::Change(color) = event;
                    if this.nommo_selected_effect() == 1 && color.is_none() {
                        return;
                    }
                    let value = color
                        .map(|color| {
                            let color = Rgba::from(color);
                            format!(
                                "#{:02x}{:02x}{:02x}",
                                (color.r * 255.).round() as u8,
                                (color.g * 255.).round() as u8,
                                (color.b * 255.).round() as u8
                            )
                        })
                        .unwrap_or_else(|| "no-color".into());
                    this.change_nommo_setting(
                        if channel == 0 { "color1" } else { "color2" },
                        json!(value),
                        window,
                        cx,
                    );
                },
            ));
        }
        self.subscriptions.push(cx.subscribe_in(
            &boost,
            window,
            |this, _, event: &StepperEvent, window, cx| {
                if this.syncing || this.nommo_selected_effect() != 12 {
                    return;
                }
                if event.value.is_finite() {
                    this.change_nommo_setting(
                        "colorBoost",
                        json!((event.value.clamp(0.25, 4.) * 4.).ceil() / 4.),
                        window,
                        cx,
                    );
                }
            },
        ));
    }
    pub(super) fn sync_nommo_effects(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.nommo_effects else {
            return;
        };
        let id = self.nommo_selected_effect();
        state.effect.update(cx, |effect, cx| {
            effect.set_selected_value(&id.to_string(), window, cx)
        });
        let setting = self.nommo_setting();
        for (channel, color) in state.colors.iter().enumerate() {
            let value = setting[if channel == 0 { "color1" } else { "color2" }]
                .as_str()
                .and_then(|s| s.strip_prefix('#'))
                .filter(|s| s.len() == 6)
                .and_then(|s| u32::from_str_radix(s, 16).ok());
            color.update(cx, |color, cx| {
                if let Some(value) = value {
                    color.set_value(rgb(value), window, cx)
                } else {
                    color.clear_value(window, cx)
                }
            });
        }
        state.boost.update(cx, |input, cx| {
            input.sync_value(
                setting["colorBoost"].as_f64().unwrap_or(1.),
                0.25,
                self.nommo_advanced() || id != 12,
                window,
                cx,
            )
        });
    }
    pub(super) fn render_nommo_effects(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let advanced = self.nommo_advanced();
        let tabs = pill("nommo-effect-tabs", false, window, cx).children(
            [(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(|(value, label)| {
                tab(label, advanced == value, t(label), false, window, cx).on_click(cx.listener(
                    move |this, _, window, cx| {
                        // iP/Tp invokes toggleTab even on the currently active pill.
                        this.draft["nommoEffects"]["advanced"] = json!(!this.nommo_advanced());
                        this.sync(window, cx);
                        cx.emit(AudioProductChanged);
                        cx.notify();
                    },
                ))
            }),
        );
        surface::panel_with_control(
            t("EFFECTS"),
            surface::help_control("nommo-effects-help", t("EFFECTS_TOOLTIP")),
            cx,
        )
        .text_color(Colors::text())
        .text_size(surface::css(14.))
        .child(div().flex().child(tabs))
        .child(if advanced {
            self.nommo_advanced_effects(cx)
        } else {
            self.nommo_quick_effects(window, cx)
        })
        .into_any_element()
    }
    fn nommo_quick_effects(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = self.nommo_effects.as_ref().expect("Nommo state");
        v_flex()
            .pt(surface::css(20.))
            .child(
                div()
                    .mb(surface::css(20.))
                    .line_height(surface::css(17.))
                    .child(t("QUICK_EFFECTS_MSG")),
            )
            .child(
                h_flex()
                    .child(
                        div()
                            .w(surface::css(150.))
                            .mr(surface::css(20.))
                            .flex_shrink_0()
                            .child(
                                surface::select(&state.effect)
                                    .items(choices(self.spec.product_id))
                                    .accessibility_label(t("QUICK_EFFECTS"))
                                    .w_full(),
                            ),
                    )
                    // Source needs more than one *connected* Chroma device. Catalog entries
                    // are not connected-device evidence; keep the honest no-peer branch.
                    .child(
                        BaseButton::new("nommo-chroma-sync")
                            .disabled(true)
                            .p_0()
                            .flex()
                            .items_center()
                            .opacity(0.3)
                            .accessibility_label(t("ONLY_ONE_CHROMA_DEVICE"))
                            .child(
                                img("synapse/chroma-sync.svg")
                                    .size(surface::css(26.))
                                    .mr(surface::css(10.)),
                            )
                            .child(
                                div()
                                    .w(surface::css(340.))
                                    .underline()
                                    .child(t("ONLY_ONE_CHROMA_DEVICE")),
                            ),
                    ),
            )
            .child(self.nommo_parameters(window, cx))
            .into_any_element()
    }
    fn nommo_parameters(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = self.nommo_effects.as_ref().expect("Nommo state");
        let id = self.nommo_selected_effect();
        match id {
            1 | 2 | 19 => {
                let random = self.nommo_setting()["isRandom"] == true;
                let mut controls = h_flex().items_start();
                for channel in 0..if id == 1 { 1 } else { 2 } {
                    let label = if id == 1 {
                        t("COLOR")
                    } else {
                        t("COLOR_DROP_NAME").replace("{{num}}", &(channel + 1).to_string())
                    };
                    controls = controls.child(
                        v_flex()
                            .mt(surface::css(20.))
                            .mr(surface::css(20.))
                            .child(label.clone())
                            .child(
                                div().mt(surface::css(5.)).child(
                                    LightingColorPicker::new(&state.colors[channel], label)
                                        .allow_none(id != 1)
                                        .disabled(random),
                                ),
                            ),
                    );
                }
                if id != 1 {
                    controls = controls.child(
                        div().mt(surface::css(45.)).child(
                            surface::check_item(
                                "nommo-random",
                                t("RANDOM_COLOR"),
                                random,
                                false,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.change_nommo_setting(
                                        "isRandom",
                                        json!(!random),
                                        window,
                                        cx,
                                    )
                                },
                            )),
                        ),
                    );
                }
                v_flex()
                    .child(controls)
                    .when(id == 19, |view| {
                        view.child(self.nommo_direction(true, window, cx))
                    })
                    .into_any_element()
            }
            4 => self.nommo_direction(false, window, cx),
            12 => v_flex()
                .mt(surface::css(20.))
                .child(t("TEXT_COLOR_BOOST"))
                .child(div().mt(surface::css(10.)).child(state.boost.clone()))
                .into_any_element(),
            // Spectrum has no mounted parameter control in either source.
            _ => div().into_any_element(),
        }
    }
    fn nommo_direction(
        &self,
        tidal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.nommo_setting()["direction"].as_i64();
        let directions = if tidal {
            [(1, "out"), (0, "in")]
        } else {
            [(11, "cw"), (12, "ccw")]
        };
        let mut tabs = pill("nommo-direction", true, window, cx);
        for (direction, name) in directions {
            let selected = value == Some(direction);
            let label = t(if tidal {
                "TEXT_DIRECTION"
            } else if direction == 11 {
                "CLOCKWISE"
            } else {
                "COUNTER_CLOCKWISE"
            });
            tabs = tabs.child(
                tab(name, selected, label, true, window, cx)
                    .child(
                        img(format!(
                            "synapse/direction-{name}{}.svg",
                            if selected { "-active" } else { "" }
                        ))
                        .size(surface::css(20.)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let next = if value == Some(directions[0].0) {
                            directions[1].0
                        } else {
                            directions[0].0
                        };
                        this.change_nommo_setting("direction", json!(next), window, cx)
                    })),
            );
        }
        v_flex()
            .mt(surface::css(if tidal { 10. } else { 20. }))
            .child(t("TEXT_DIRECTION"))
            .child(div().flex().mt(surface::css(5.)).child(tabs))
            .into_any_element()
    }
    fn nommo_advanced_effects(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.nommo_effects.as_ref().expect("Nommo state");
        // Mm/fM installed-branch layout. um/mM requests Studio through the
        // Chroma host. The local Dashboard cannot fulfill that Studio target.
        v_flex()
            .pt(surface::css(20.))
            .line_height(surface::css(17.))
            .child(
                div()
                    .line_height(surface::css(18.))
                    .child(t("ADVANCED_EFFECTS_MSG")),
            )
            .child(
                div()
                    .mt(surface::css(20.))
                    .child(t("NO_CHROMA_STUDIO_PROFILE_TEXT")),
            )
            .child(
                div().mt(surface::css(5.)).child(
                    surface::select(&state.chroma_profiles)
                        .items(vec![])
                        .disabled(true)
                        .w_full(),
                ),
            )
            .child(
                BaseButton::new("nommo-launch-chroma")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(AudioStudioRequested)))
                    .mt(surface::css(20.))
                    .w(surface::css(240.))
                    .h(surface::css(50.))
                    .border_1()
                    .border_color(Colors::text())
                    .rounded(surface::css(3.))
                    .bg(Colors::surface())
                    .hover(|style| style.bg(Colors::launch_hover()))
                    .active(|style| style.bg(Colors::surface()))
                    .flex()
                    .items_center()
                    .px(surface::css(16.))
                    .gap(surface::css(10.))
                    .accessibility_label(t("LAUNCH_CHROMA_STUDIO"))
                    .child(
                        img("synapse/hue-logo_chromastudio.svg")
                            .size(surface::css(30.))
                            .flex_shrink_0(),
                    )
                    .child(t("LAUNCH_CHROMA_STUDIO").to_uppercase()),
            )
            .into_any_element()
    }
}

#[derive(Default)]
struct PillPointer {
    hovered: bool,
    pressed: bool,
}

fn pill(id: &'static str, direction: bool, window: &mut Window, cx: &mut App) -> Tabs {
    let pointer =
        window.use_keyed_state((ElementId::from(id), "nommo-pill-pointer"), cx, |_, _| {
            PillPointer::default()
        });
    let hovered = pointer.read(cx).hovered;
    let pressed = pointer.read(cx).pressed;
    let border = motion::transition(
        (id, "nommo-pill-border"),
        CssColor(
            if hovered {
                Colors::selected()
            } else {
                Colors::border()
            }
            .into(),
        ),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    let background = motion::transition(
        (id, "nommo-pill-background"),
        CssColor(
            if pressed {
                Colors::pill_pressed()
            } else {
                Colors::surface()
            }
            .into(),
        ),
        Transition::new(Duration::from_millis(500)).easing(Easing::Ease),
        window,
        cx,
    );
    Tabs::new(id)
        .flex()
        .items_center()
        .w_auto()
        .h(surface::css(if direction { 42. } else { 36. }))
        .p(surface::css(5.))
        .gap(surface::css(if direction { 5. } else { 0. }))
        .border_1()
        .border_color(border.0)
        .rounded(surface::css(if direction { 20. } else { 18. }))
        .bg(background.0)
        .on_hover(window.listener_for(&pointer, |pointer, hovered, _, cx| {
            pointer.hovered = *hovered;
            cx.notify();
        }))
        .capture_any_mouse_down(window.listener_for(
            &pointer,
            |pointer, event: &MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    pointer.pressed = true;
                    cx.notify();
                }
            },
        ))
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&pointer, |pointer, _, _, cx| {
                pointer.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&pointer, |pointer, _, _, cx| {
                pointer.pressed = false;
                cx.notify();
            }),
        )
}
fn tab(
    id: &'static str,
    selected: bool,
    label: String,
    direction: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let group = if direction {
        "nommo-direction"
    } else {
        "nommo-effect-tabs"
    };
    let pointer = window.use_keyed_state(
        (ElementId::from(group), "nommo-pill-pointer"),
        cx,
        |_, _| PillPointer::default(),
    );
    let pressed = pointer.read(cx).pressed;
    let background = motion::transition(
        (ElementId::from(id), "nommo-background"),
        CssColor(
            if selected {
                Colors::selected()
            } else if pressed {
                Colors::pressed()
            } else {
                Colors::surface()
            }
            .into(),
        ),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    let text = motion::transition(
        (ElementId::from(id), "nommo-text"),
        CssColor(
            if selected {
                Colors::surface()
            } else {
                Colors::text()
            }
            .into(),
        ),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    BaseButton::new(id)
        .role(Role::Tab)
        .selected(selected)
        .accessibility_label(label.clone())
        .h(surface::css(if direction { 30. } else { 26. }))
        .px(surface::css(10.))
        .py_0()
        .rounded(surface::css(if direction { 15. } else { 13. }))
        .bg(background.0)
        .text_color(text.0)
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .flex()
        .items_center()
        .justify_center()
        .when(direction, |button| button.w(surface::css(40.)))
        .when(!direction, |button| button.child(label))
}
