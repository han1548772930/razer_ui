//! Current 4115 Tm/Em/Ap mounted tree and the five non-BLE quick-effect children.
//! Native software frames and host storage observations have separate owners.
use super::chroma_runtime::ControllerChromaCommand;
use super::quick_lighting_state::{QuickColor, QuickEffectId, QuickSetting};
use super::*;
use crate::features::{controls::Choice, lighting_color::LightingColorPicker};
use gpui_kit::component::{
    color_picker::{ColorPickerEvent, ColorPickerState},
    select::{SelectEvent, SelectState},
};
use razer_device::controller_lighting::{LightingOperation, QuickEffectLevel, WaveDirection};
use razer_widgets::source_tooltip::{SourceTooltip, SourceTooltipKind};
use razer_widgets::{
    synapse_select::select,
    theme::{GamepadQuickLightingColors, KeyboardRgbEffectColors as Colors},
};

pub(super) struct QuickLightingUi {
    effect: Entity<SelectState<Vec<Choice>>>,
    chroma_profile: Entity<SelectState<Vec<Choice>>>,
    colors: [Entity<ColorPickerState>; 2],
}

fn effect_choices() -> Vec<Choice> {
    QuickEffectId::ORDER
        .into_iter()
        .map(|id| Choice::localized(u8::from(id).to_string(), id.label()))
        .collect()
}

impl GamepadProductWorkspace {
    pub fn apply_controller_sync_quick(
        &mut self,
        source_effect: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if self.spec.product_id != 4115 {
            return Ok(false);
        }
        let settings = self
            .lighting_runtime
            .quick
            .as_mut()
            .ok_or_else(|| "Kitsune quick settings are missing".to_string())?;
        if !settings
            .apply_sync(source_effect)
            .map_err(|error| format!("Invalid synchronized Kitsune effect: {error:#}"))?
        {
            return Ok(false);
        }
        self.dismiss_quick_lighting_controls(cx);
        self.chroma_runtime.advanced = false;
        self.chroma_runtime.observation =
            self.chroma_runtime.observation.clone().with_enabled(false);
        self.submit_quick_lighting(cx);
        self.sync_quick_lighting_controls(window, cx);
        Ok(true)
    }
    pub(super) fn initialize_quick_lighting_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 4115 {
            return;
        }
        razer_state::CustomColors::ensure(cx);
        let effect = cx.new(|cx| SelectState::new(effect_choices(), None, window, cx));
        let chroma_profile = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let colors = std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx)));
        self.subscriptions.push(
            cx.subscribe_in(&effect, window, |this, _, event, window, cx| {
                if this.syncing || this.quick_lighting_disabled() {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    if let Ok(id) = id
                        .parse::<u8>()
                        .map_err(anyhow::Error::from)
                        .and_then(QuickEffectId::try_from)
                    {
                        this.dismiss_quick_lighting_controls(cx);
                        if let Some(settings) = &mut this.lighting_runtime.quick {
                            settings.select(id);
                        }
                        this.submit_quick_lighting(cx);
                        this.sync_quick_lighting_controls(window, cx);
                    }
                }
            }),
        );
        self.subscriptions.push(cx.subscribe_in(
            &chroma_profile,
            window,
            |this, _, event, _, cx| {
                if this.syncing
                    || !this.checked("/profile/brightness/isEnabled")
                    || this.chroma_runtime.observation.visualizer_enabled() == Some(true)
                {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    if this.chroma_runtime.observation.select_profile(id) {
                        this.request_controller_chroma(
                            ControllerChromaCommand::SelectProfile { id: id.clone() },
                            cx,
                        );
                        cx.notify();
                    }
                }
            },
        ));
        for (channel, color) in colors.iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                color,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing || this.quick_lighting_disabled() {
                        return;
                    }
                    let Some(mut setting) =
                        this.lighting_runtime.quick.as_ref().map(|s| s.setting())
                    else {
                        return;
                    };
                    if matches!(setting, QuickSetting::Breathing { random: true, .. }) {
                        return;
                    }
                    let ColorPickerEvent::Change(color) = event;
                    let next = color
                        .map(|color| {
                            let color = Rgba::from(color);
                            QuickColor::Rgb(super::lighting_state::HexColor([
                                (color.r * 255.).round() as u8,
                                (color.g * 255.).round() as u8,
                                (color.b * 255.).round() as u8,
                            ]))
                        })
                        .unwrap_or(QuickColor::None);
                    if let Err(error) = setting.set_color(channel, next) {
                        eprintln!("[controller-lighting] {error:#}");
                        return;
                    }
                    this.store_quick_lighting(setting, window, cx);
                },
            ));
        }
        self.quick_lighting_ui = Some(QuickLightingUi {
            effect,
            chroma_profile,
            colors,
        });
        self.sync_quick_lighting_controls(window, cx);
    }
    pub(super) fn sync_quick_lighting_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ui) = &self.quick_lighting_ui else {
            return;
        };
        let Some(settings) = &self.lighting_runtime.quick else {
            return;
        };
        let setting = settings.setting();
        let selected = u8::from(settings.selected).to_string();
        let profiles = self
            .chroma_runtime
            .observation
            .profiles()
            .unwrap_or(&[])
            .iter()
            .map(|p| Choice::new(p.id(), p.name().to_owned()))
            .collect();
        let selected_profile = self
            .chroma_runtime
            .observation
            .selected_profile()
            .map(str::to_owned);
        let was_syncing = self.syncing;
        self.syncing = true;
        ui.effect.update(cx, |state, cx| {
            state.set_items(effect_choices(), window, cx);
            state.set_selected_value(&selected, window, cx);
        });
        ui.chroma_profile.update(cx, |state, cx| {
            state.set_items(profiles, window, cx);
            if let Some(id) = selected_profile {
                state.set_selected_value(&id, window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
        for (channel, color) in ui.colors.iter().enumerate() {
            let rgb = setting.color(channel).and_then(QuickColor::rgb);
            color.update(cx, |state, cx| {
                if let Some([r, g, b]) = rgb {
                    state.set_value(
                        gpui_kit::rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)),
                        window,
                        cx,
                    );
                } else {
                    state.clear_value(window, cx);
                }
            });
        }
        self.syncing = was_syncing;
    }
    pub(super) fn dismiss_quick_lighting_controls(&self, cx: &mut Context<Self>) {
        if let Some(ui) = &self.quick_lighting_ui {
            for color in &ui.colors {
                color.update(cx, |state, cx| state.set_open(false, cx));
            }
        }
    }
    pub(super) fn project_quick_lighting(&mut self) {
        if let Some(settings) = &self.lighting_runtime.quick {
            self.draft["profile"]["quickEffects"] =
                serde_json::to_value(settings).expect("typed Kitsune quick effects");
        }
    }
    fn commit_quick_custom_colors(
        &mut self,
        edit: razer_model::preferences::CustomColorEdit,
        cx: &mut Context<Self>,
    ) {
        let Some(settings) = &mut self.lighting_runtime.quick else {
            return;
        };
        let colors = match settings.edit_custom_color(edit) {
            Ok(colors) => colors,
            Err(error) => {
                eprintln!("[controller-lighting] Invalid custom-color edit: {error:#}");
                return;
            }
        };
        self.project_quick_lighting();
        cx.emit(GamepadProductChanged);
        self.request_controller_chroma(ControllerChromaCommand::CustomColors(colors), cx);
        cx.notify();
    }
    pub(super) fn submit_quick_lighting(&mut self, cx: &mut Context<Self>) {
        self.project_quick_lighting();
        if !self.chroma_runtime.advanced {
            let operation = (|| {
                let effect = self
                    .lighting_runtime
                    .quick
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Kitsune quick settings are missing"))?
                    .setting()
                    .software();
                let edition = u8::try_from(self.edition_id)?;
                let operation = LightingOperation::Software { edition, effect };
                operation.validate_product(self.spec.product_id)?;
                Ok(operation)
            })();
            self.enqueue_lighting(operation, cx);
        }
        cx.emit(GamepadProductChanged);
        cx.notify();
    }
    fn store_quick_lighting(
        &mut self,
        setting: QuickSetting,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = &mut self.lighting_runtime.quick {
            state.store(setting);
        }
        self.submit_quick_lighting(cx);
        self.sync_quick_lighting_controls(window, cx);
    }
    fn quick_lighting_disabled(&self) -> bool {
        self.chroma_runtime.advanced
            || !self.checked("/profile/brightness/isEnabled")
            || self.chroma_runtime.observation.quick_disabled()
    }

    pub(super) fn kitsune_lighting(
        &self,
        surface: LightingSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .relative()
            .w_full()
            .min_h_full()
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex()
                            .child(self.controller_brightness_panel(window, cx))
                            .child(self.switch_off_lighting(window, cx)),
                    ))
                    .child(surface::page_column(self.kitsune_effects_panel(window, cx))),
            )
            .when(
                self.chroma_runtime.observation.dynamic_lighting() == Some(true),
                |body| body.child(self.controller_dynamic_warning(surface, cx)),
            )
            .into_any_element()
    }
    fn kitsune_effects_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.checked("/profile/brightness/isEnabled");
        let advanced = self.chroma_runtime.advanced;
        let disabled =
            self.chroma_runtime.observation.visualizer_enabled() == Some(true) || !enabled;
        surface::panel_with_title_switch(
            t("EFFECTS"),
            surface::SynapseSwitch::new("kitsune-effects-enabled")
                .checked(enabled)
                .accessibility_label(t("EFFECTS"))
                .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                    this.write("/profile/brightness/isEnabled", json!(*enabled), cx)
                })),
            surface::help_control("kitsune-effects-help", t("EFFECTS_TOOLTIP")),
            cx,
        )
        .child(
            v_flex()
                .opacity(if enabled { 1. } else { 0.3 })
                .child(
                    BaseButton::new("kitsune-effect-tab")
                        .disabled(disabled)
                        .self_start()
                        .flex()
                        .items_center()
                        .h(surface::css(36.))
                        .p(surface::css(5.))
                        .mb(surface::css(20.))
                        .rounded(surface::css(18.))
                        .border_1()
                        .border_color(Colors::border())
                        .hover(|style| style.border_color(Colors::accent()))
                        .bg(Colors::background())
                        .children([(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(
                            |(value, label)| {
                                div()
                                    .h(surface::css(26.))
                                    .px(surface::css(10.))
                                    .py(surface::css(5.))
                                    .rounded(surface::css(13.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(17.))
                                    .bg(if advanced == value {
                                        Colors::accent()
                                    } else {
                                        Colors::background()
                                    })
                                    .text_color(if advanced == value {
                                        Colors::background()
                                    } else {
                                        Colors::text()
                                    })
                                    .child(t(label))
                            },
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.controller_advanced_changed(
                                !this.chroma_runtime.advanced,
                                window,
                                cx,
                            )
                        })),
                )
                .child(if advanced {
                    self.kitsune_advanced_effects(disabled, cx)
                } else {
                    self.kitsune_quick_effects(window, cx)
                }),
        )
        .when(
            self.chroma_runtime.observation.visualizer_enabled() == Some(true),
            |panel| panel.child(self.kitsune_app_status(cx)),
        )
        .into_any_element()
    }
    fn kitsune_quick_effects(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (Some(ui), Some(state)) = (&self.quick_lighting_ui, &self.lighting_runtime.quick)
        else {
            return div().into_any_element();
        };
        let disabled = self.quick_lighting_disabled();
        let can_sync = self
            .chroma_runtime
            .observation
            .connected_products()
            .is_some_and(|products| products.len() > 1 && products.iter().any(|pid| *pid != 4115));
        let setting = state.setting();
        let mut body =
            v_flex()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(Colors::text())
                .opacity(if self.chroma_runtime.observation.quick_disabled() {
                    0.3
                } else {
                    1.
                })
                .child(div().mb(surface::css(20.)).child(t("QUICK_EFFECTS_MSG")))
                .child(
                    h_flex()
                        .items_center()
                        .child(
                            select(&ui.effect)
                                .items(effect_choices())
                                .disabled(disabled)
                                .w(surface::css(150.))
                                .mr(surface::css(20.))
                                .accessibility_label(t("QUICK_EFFECTS")),
                        )
                        .child(
                            BaseButton::new("kitsune-sync-quick")
                                .disabled(disabled || !can_sync)
                                .flex()
                                .items_center()
                                .text_size(surface::css(14.))
                                .text_color(Colors::text())
                                .opacity(if can_sync { 1. } else { 0.3 })
                                .hover(|style| style.text_color(Colors::accent()))
                                .child(self.kitsune_sync_icon())
                                .child(div().w(surface::css(340.)).underline().child(t(
                                    if can_sync {
                                        "APPLY_TO_ALL_CHROMA_DEVICES"
                                    } else {
                                        "ONLY_ONE_CHROMA_DEVICE"
                                    },
                                )))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if !this.chroma_runtime.sync_started.is_some_and(|start| {
                                        start.elapsed() < std::time::Duration::from_millis(1200)
                                    }) {
                                        this.chroma_runtime.sync_started =
                                            Some(std::time::Instant::now());
                                        this.chroma_runtime.sync_generation =
                                            this.chroma_runtime.sync_generation.wrapping_add(1);
                                    }
                                    if let Some(settings) = &this.lighting_runtime.quick {
                                        let source_effect = settings.sync_payload();
                                        this.request_controller_chroma(
                                            ControllerChromaCommand::SyncQuick { source_effect },
                                            cx,
                                        );
                                    }
                                    cx.notify();
                                })),
                        ),
                );
        match &setting {
            QuickSetting::Breathing { random, .. } => {
                body = body.child(
                    h_flex()
                        .items_end()
                        .flex_wrap()
                        .gap(surface::css(10.))
                        .mt(surface::css(20.))
                        .mb(surface::css(20.))
                        .children((0..2).map(|channel| {
                            self.kitsune_color(channel, true, disabled || *random, cx)
                        }))
                        .child(
                            surface::check_item(
                                "kitsune-breathing-random",
                                t("RANDOM_COLOR"),
                                *random,
                                disabled,
                                window,
                                cx,
                            )
                            .ml(surface::css(10.))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    if let Some(mut setting) =
                                        this.lighting_runtime.quick.as_ref().map(|s| s.setting())
                                    {
                                        if let QuickSetting::Breathing { random, .. } = &mut setting
                                        {
                                            *random = !*random;
                                        }
                                        this.store_quick_lighting(setting, window, cx);
                                    }
                                },
                            )),
                        ),
                );
            }
            QuickSetting::Static { .. } | QuickSetting::Reactive { .. } => {
                body = body.child(
                    div()
                        .mt(surface::css(20.))
                        .child(self.kitsune_color(0, false, disabled, cx)),
                );
            }
            QuickSetting::Wave { direction, .. } => {
                body = body.child(self.kitsune_wave_direction(*direction, disabled, cx));
            }
            QuickSetting::Spectrum { .. } => {}
        }
        if let Some(level) = setting.level() {
            body = body.child(self.kitsune_level(
                level,
                state.selected == QuickEffectId::Wave,
                disabled,
                cx,
            ));
        }
        body.into_any_element()
    }
    fn kitsune_sync_icon(&self) -> AnyElement {
        if self
            .chroma_runtime
            .sync_started
            .is_some_and(|start| start.elapsed() < std::time::Duration::from_millis(1200))
        {
            div()
                .size(surface::css(26.))
                .mr(surface::css(10.))
                .with_animation(
                    (
                        "kitsune-chroma-sync-turn",
                        self.chroma_runtime.sync_generation,
                    ),
                    Animation::new(std::time::Duration::from_millis(1200))
                        .with_easing(gpui_kit::linear),
                    |icon, phase| {
                        icon.child(
                            img(SharedString::from(format!(
                                "synapse/keyboard-chroma-sync-{:02}.png",
                                ((phase * 108.).floor() as usize) % 36
                            )))
                            .size_full(),
                        )
                    },
                )
                .into_any_element()
        } else {
            img("synapse/chroma-sync.svg")
                .size(surface::css(26.))
                .mr(surface::css(10.))
                .into_any_element()
        }
    }
    fn kitsune_color(
        &self,
        channel: usize,
        multiple: bool,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let ui = self
            .quick_lighting_ui
            .as_ref()
            .expect("mounted Kitsune controls");
        let label = if multiple {
            t("COLOR_DROP_NAME").replace("{{num}}", &(channel + 1).to_string())
        } else {
            t("COLOR")
        };
        v_flex()
            .child(label.clone())
            .child(
                div().mt(surface::css(5.)).child(
                    LightingColorPicker::new(&ui.colors[channel], label)
                        .allow_none(multiple)
                        .disabled(disabled)
                        .custom_colors(
                            self.lighting_runtime
                                .quick
                                .as_ref()
                                .expect("mounted Kitsune quick settings")
                                .custom_color_slots(),
                        )
                        .on_custom_color_edit(cx.listener(
                            |this, edit: &razer_model::preferences::CustomColorEdit, _, cx| {
                                this.commit_quick_custom_colors(*edit, cx)
                            },
                        )),
                ),
            )
            .into_any_element()
    }
    fn kitsune_level(
        &self,
        level: QuickEffectLevel,
        speed: bool,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let labels = if speed {
            ["SLOW", "TEXT_SLIDER_MEDIUM", "FAST"]
        } else {
            [
                "TEXT_SLIDER_SHORT",
                "TEXT_SLIDER_MEDIUM",
                "TEXT_SLIDER_LONG",
            ]
        };
        v_flex()
            .mt(surface::css(20.))
            .child(
                div()
                    .mb(surface::css(10.))
                    .line_height(surface::css(14.))
                    .child(t(if speed { "TEXT_SPEED" } else { "TEXT_DURATION" }).to_uppercase()),
            )
            .child(
                h_flex().gap(surface::css(5.)).children(
                    [
                        QuickEffectLevel::One,
                        QuickEffectLevel::Two,
                        QuickEffectLevel::Three,
                    ]
                    .into_iter()
                    .zip(labels)
                    .map(|(value, label)| {
                        let selected = level == value;
                        BaseButton::new(("kitsune-effect-level", u64::from(value.source_value())))
                            .disabled(disabled)
                            .accessibility_label(t(label))
                            .min_w(surface::css(90.))
                            .h(surface::css(27.))
                            .px(surface::css(16.))
                            .pt(surface::css(7.))
                            .pb(surface::css(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .text_color(if disabled {
                                GamepadQuickLightingColors::disabled_text()
                            } else {
                                GamepadQuickLightingColors::text()
                            })
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(if selected {
                                Colors::accent()
                            } else {
                                Colors::border()
                            })
                            .bg(if selected {
                                GamepadQuickLightingColors::active_background()
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .hover(|style| style.bg(Colors::hover()))
                            .child(t(label).to_uppercase())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(mut setting) =
                                    this.lighting_runtime.quick.as_ref().map(|s| s.setting())
                                {
                                    setting.set_level(value);
                                    this.store_quick_lighting(setting, window, cx);
                                }
                            }))
                    }),
                ),
            )
            .into_any_element()
    }
    fn kitsune_wave_direction(
        &self,
        direction: WaveDirection,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .mt(surface::css(20.))
            .child(t("TEXT_DIRECTION"))
            .child(
                h_flex()
                    .mt(surface::css(5.))
                    .h(surface::css(42.))
                    .p(surface::css(5.))
                    .self_start()
                    .gap(surface::css(5.))
                    .rounded(surface::css(20.))
                    .border_1()
                    .border_color(Colors::border())
                    .bg(Colors::background())
                    .hover(|style| style.border_color(Colors::accent()))
                    .children(
                        [
                            (WaveDirection::Left, "left"),
                            (WaveDirection::Right, "right"),
                        ]
                        .map(|(value, name)| {
                            let selected = direction == value;
                            BaseButton::new((
                                "kitsune-wave-direction",
                                if value == WaveDirection::Left { 1_u64 } else { 2 },
                            ))
                            .disabled(disabled)
                            .accessibility_label(t(if value == WaveDirection::Left {
                                "DIRECTION_LEFT"
                            } else {
                                "DIRECTION_RIGHT"
                            }))
                            .w(surface::css(40.))
                            .h(surface::css(30.))
                            .rounded(surface::css(15.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if selected {
                                Colors::accent()
                            } else {
                                Colors::background()
                            })
                            .child(
                                img(SharedString::from(format!(
                                    "synapse/direction-{name}{}.svg",
                                    if selected { "-active" } else { "" }
                                )))
                                .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    if let Some(mut setting) =
                                        this.lighting_runtime.quick.as_ref().map(|s| s.setting())
                                    {
                                        if let QuickSetting::Wave { direction, .. } = &mut setting {
                                            *direction = if *direction == WaveDirection::Left {
                                                WaveDirection::Right
                                            } else {
                                                WaveDirection::Left
                                            };
                                        }
                                        this.store_quick_lighting(setting, window, cx);
                                    }
                                },
                            ))
                        }),
                    ),
            )
            .into_any_element()
    }
    fn kitsune_advanced_effects(&self, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let observation = &self.chroma_runtime.observation;
        // Qp is invisible until installation observation completes. Its online
        // installer branch is outside the local product scope.
        if observation.fully_installed() != Some(true)
            || observation.module_installed() != Some(true)
        {
            return div().into_any_element();
        }
        let Some(ui) = &self.quick_lighting_ui else {
            return div().into_any_element();
        };
        let profiles = observation.profiles().unwrap_or(&[]);
        v_flex()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(Colors::text())
            .child(t("ADVANCED_EFFECTS_MSG"))
            .child(div().mt(surface::css(20.)).child(t(if profiles.is_empty() {
                "NO_CHROMA_STUDIO_PROFILE_TEXT"
            } else {
                "CHROMA_STUDIO"
            })))
            .child(
                select(&ui.chroma_profile)
                    .items(
                        profiles
                            .iter()
                            .map(|p| Choice::new(p.id(), p.name().to_owned()))
                            .collect(),
                    )
                    .disabled(disabled || profiles.is_empty())
                    .mt(surface::css(5.))
                    .w(surface::css(200.))
                    .accessibility_label(t("CHROMA_STUDIO")),
            )
            .child(
                BaseButton::new("kitsune-launch-studio")
                    .disabled(disabled)
                    .mt(surface::css(20.))
                    .flex()
                    .items_center()
                    .w(surface::css(240.))
                    .h(surface::css(50.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(Colors::text())
                    .bg(Colors::background())
                    .hover(|style| style.bg(Colors::black()))
                    .text_color(Colors::text())
                    .text_size(surface::css(14.))
                    .child(
                        img("synapse/gamepad-lighting/studio.svg")
                            .size(surface::css(30.))
                            .ml(surface::css(16.))
                            .mr(surface::css(10.)),
                    )
                    .child(t("LAUNCH_CHROMA_STUDIO").to_uppercase())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_controller_chroma(ControllerChromaCommand::LaunchStudio, cx)
                    })),
            )
            .into_any_element()
    }
    fn kitsune_app_status(&self, cx: &Context<Self>) -> AnyElement {
        let name = self.chroma_runtime.observation.app_name().unwrap_or("");
        let icon = if name == "Chroma Visualizer App" {
            Some("synapse/gamepad-lighting/visualizer.svg")
        } else {
            self.chroma_runtime.observation.app_icon()
        };
        let name = name
            .to_lowercase()
            .split(' ')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
            .replace(".exe", "");
        let disabled = !self.checked("/profile/brightness/isEnabled");
        let manage = BaseButton::new("kitsune-manage-apps")
            .disabled(disabled)
            .size(surface::css(14.))
            .p_0()
            .cursor_pointer()
            .accessibility_label(t("MANAGE_CHROMA_APPS"))
            .child(img("synapse/gamepad-lighting/settings.svg").size_full())
            .on_click(cx.listener(|this, _, _, cx| {
                this.request_controller_chroma(ControllerChromaCommand::ManageApps, cx)
            }));
        div()
            .relative()
            .mt(surface::css(20.))
            .w_full()
            .h(surface::css(45.))
            .rounded(surface::css(5.))
            .bg(Colors::application_background())
            .text_size(surface::css(14.))
            .text_center()
            .opacity(if disabled { 0.5 } else { 1. })
            .children(icon.map(|icon| {
                img(SharedString::from(icon.to_owned()))
                    .absolute()
                    .left(surface::css(10.))
                    .top(surface::css(10.))
                    .size(surface::css(25.))
            }))
            .child(
                v_flex()
                    .w(surface::css(500.))
                    .h_full()
                    .justify_center()
                    .child(
                        div()
                            .text_color(Colors::application_label())
                            .child(format!("{}:", t("APP_IN_USE"))),
                    )
                    .child(
                        div()
                            .text_color(Colors::text())
                            .child(format!("{name} ({})", t("CHROMA_APPS"))),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(surface::css(30.))
                    .top(surface::css(15.))
                    .child(
                        SourceTooltip::new(
                            "kitsune-manage-apps-tip",
                            t("MANAGE_CHROMA_APPS"),
                            165.,
                        )
                        .kind(SourceTooltipKind::ChromaManage)
                        .trigger(move |_, _, _| manage.into_any_element()),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(15.))
                    .child(
                        SourceTooltip::new(
                            "kitsune-apps-tip",
                            format!(
                                "{}. \n{}.",
                                t("CHROMA_INDICATOR_TIP_1"),
                                t("CHROMA_INDICATOR_TIP_2")
                            ),
                            410.,
                        )
                        .kind(SourceTooltipKind::ChromaStatus)
                        .trigger(move |_, _, _| {
                            BaseButton::new("kitsune-apps-help")
                                .disabled(disabled)
                                .size(surface::css(14.))
                                .p_0()
                                .rounded_full()
                                .bg(Colors::help_background())
                                .hover(|style| style.cursor_pointer().bg(Colors::help_hover()))
                                .child(
                                    img("synapse/automation-tooltip_questionmark.svg").size_full(),
                                )
                                .into_any_element()
                        }),
                    ),
            )
            .into_any_element()
    }
}
