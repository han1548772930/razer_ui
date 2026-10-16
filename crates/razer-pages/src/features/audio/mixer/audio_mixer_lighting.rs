//! Current1342 lP/useFp/useYp/useIP/EP/useHp/useEP/useAP and Pp/Xp.
//! UI main.2623357c.js 239660–241470; CSS main.07a2c4ed.css.
//! Local fader defaults: MW main.6861ee97d479a87c4d7a.js 8385–8460.
use super::audio_mixer_theme::Colors as C;
use super::*;
use crate::features::lighting_color::LightingColorPicker;
use gpui_kit::base::{Button as BaseButton, Radio};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use razer_widgets::source_slider::SourceSlider;
#[path = "audio_mixer_lighting_preview.rs"]
mod preview;

const REGIONS: [u32; 10] = [4, 5, 6, 7, 8, 9, 16, 10, 33, 32];
const EFFECTS: [(u32, &str); 6] = [
    (2, "BREATHING"),
    (3, "SPECTRUM_CYCLING"),
    (1, "STATIC"),
    (4, "WAVE"),
    (8, "FIRE"),
    (13, "WHEEL"),
];

struct Region {
    effect: Entity<SelectState<Vec<Choice>>>,
    colors: Vec<Entity<ColorPickerState>>,
}
#[derive(Default)]
pub(super) struct State {
    regions: BTreeMap<u32, Region>,
    sliders: BTreeMap<u32, Entity<SliderState>>,
    /// Pp/Xp effectSettings survive effect switches; this is Redux view state,
    /// separate from the persisted source selectedEffectSetting.
    cached: BTreeMap<(u32, u32), Value>,
    previews: BTreeMap<(u32, usize), (String, std::sync::Arc<Image>)>,
    preview_values: BTreeMap<u32, u32>,
    resources: crate::runtime_resources::TextResources,
    foreground_colors: Option<Vec<preview::ForegroundColor>>,
}

fn effect_ids(region: u32) -> &'static [u32] {
    match region {
        4 => &[3, 1, 2],
        10 => &[3, 1],
        5 => &[2, 3, 1, 4, 8, 13],
        6..=9 => &[2, 3, 1, 4],
        _ => &[1],
    }
}
fn default_setting(region: u32, effect: u32) -> Value {
    match effect {
        1 if matches!(region, 5 | 10) => json!({"color1":vec!["#00ff00";4]}),
        1 if region == 16 => {
            json!({"color1":["#00ff00","#ff0000","#00ff00","#ff0000","#00ff00","#ff0000","#00ff00","#ff0000"]})
        }
        1 if matches!(region, 32 | 33) => json!({"color1":["#00ff00","#ff0000"]}),
        1 | 12 => json!({"color1":"#00ff00"}),
        2 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false}),
        4 => json!({"direction":2}),
        13 => json!({"direction":1}),
        3 if region == 10 => json!({"color1":vec!["#ff18ff";4]}),
        _ => json!({}),
    }
}
fn brightness_index(region: u32) -> usize {
    REGIONS
        .iter()
        .position(|r| *r == region)
        .expect("source region")
}
fn is_fader(region: u32) -> bool {
    (6..=9).contains(&region)
}
fn effect_path(region: u32) -> String {
    if is_fader(region) {
        format!("/device/fader/quickEffects/{}", region - 6)
    } else {
        format!("/profile/quickEffects/{}", brightness_index(region))
    }
}
fn default_fader() -> Value {
    json!({"isEnabled":false,"useMode":0,
        "quickEffects":(6..=9).map(|region|json!({"regionId":region,"selectedEffectId":3})).collect::<Vec<_>>(),
        "meterEffects":(6..=9).map(|region|json!({"regionId":region,"selectedEffectId":12,"selectedEffectSetting":{"color1":"#00ff00"}})).collect::<Vec<_>>(),
        "dynamicSpeeds":(6..=9).map(|region|json!({"regionId":region,"speed":2})).collect::<Vec<_>>()})
}

impl AudioProductWorkspace {
    pub(super) fn initialize_mixer_lighting_draft(&mut self) {
        if self.spec.product_id == 1342 {
            self.mixer_lighting.preview_values.clear();
            self.draft["device"]["fader"] = default_fader();
        }
    }
    pub(super) fn restore_mixer_lighting(&mut self, saved: Option<&Value>) {
        if self.spec.product_id != 1342 {
            return;
        }
        let Some(saved) = saved else {
            return;
        };
        // Generic merge_known deliberately drops unknown object keys; source
        // quick-effect settings change shape when the selected effect changes.
        if let Some(effects) = saved["profile"]["quickEffects"].as_array() {
            for entry in effects {
                if let Some(region) = entry["regionId"]
                    .as_u64()
                    .map(|r| r as u32)
                    .filter(|r| REGIONS.contains(r))
                {
                    self.draft["profile"]["quickEffects"][brightness_index(region)] = entry.clone();
                }
            }
        }
        if saved["device"]["fader"].is_object() {
            for field in [
                "isEnabled",
                "useMode",
                "quickEffects",
                "meterEffects",
                "dynamicSpeeds",
            ] {
                if let Some(value) = saved["device"]["fader"].get(field) {
                    self.draft["device"]["fader"][field] = value.clone();
                }
            }
        }
    }
    fn lighting_enabled(&self, region: u32) -> bool {
        let region = if is_fader(region) { 6 } else { region };
        self.draft["profile"]["brightness"][brightness_index(region)]["isEnabled"]
            .as_bool()
            .unwrap_or(true)
    }
    fn lighting_effect_id(&self, region: u32) -> u32 {
        self.draft
            .pointer(&effect_path(region))
            .and_then(|v| v["selectedEffectId"].as_u64())
            .unwrap_or(if matches!(region, 16 | 32 | 33) { 1 } else { 3 }) as u32
    }
    fn lighting_setting(&self, region: u32) -> Value {
        if is_fader(region)
            && self.draft["device"]["fader"]["useMode"]
                .as_u64()
                .unwrap_or(0)
                != 0
        {
            return self.draft["device"]["fader"]["meterEffects"][(region-6) as usize]["selectedEffectSetting"].clone();
        }
        self.draft
            .pointer(&effect_path(region))
            .and_then(|v| v.get("selectedEffectSetting"))
            .cloned()
            .unwrap_or_else(|| default_setting(region, self.lighting_effect_id(region)))
    }
    pub(super) fn initialize_mixer_lighting(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        self.subscriptions
            .push(razer_assets::observe_resources(cx, |this, _, cx| {
                if this.mixer_lighting.resources.refresh() {
                    this.refresh_mixer_lighting_previews();
                    cx.notify();
                }
            }));
        for region in REGIONS {
            let options: Vec<Choice> = effect_ids(region)
                .iter()
                .map(|id| {
                    Choice::localized(
                        id.to_string(),
                        EFFECTS.iter().find(|(i, _)| i == id).unwrap().1,
                    )
                })
                .collect();
            let effect = cx.new(|cx| SelectState::new(options, None, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &effect,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing || !this.lighting_enabled(region) {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(id)) = event {
                        if let Ok(id) = id.parse::<u32>() {
                            this.change_mixer_lighting_effect(region, id, window, cx);
                        }
                    }
                },
            ));
            let mut colors = Vec::new();
            for index in 0..8 {
                let color = cx.new(|cx| ColorPickerState::new(window, cx));
                self.subscriptions.push(cx.subscribe_in(
                    &color,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing || !this.lighting_enabled(region) {
                            return;
                        }
                        let ColorPickerEvent::Change(color) = event;
                        let value = color
                            .map(|color| {
                                let c = Rgba::from(color);
                                format!(
                                    "#{:02x}{:02x}{:02x}",
                                    (c.r * 255.).round() as u8,
                                    (c.g * 255.).round() as u8,
                                    (c.b * 255.).round() as u8
                                )
                            })
                            .unwrap_or_else(|| "no-color".into());
                        this.change_mixer_lighting_color(region, index, value, window, cx);
                    },
                ));
                colors.push(color);
            }
            self.mixer_lighting
                .regions
                .insert(region, Region { effect, colors });
        }
        for region in [5, 33, 6, 7, 8, 9] {
            let (min, max, value) = if is_fader(region) {
                (1., 3., 2.)
            } else {
                (0., 100., 100.)
            };
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(1.)
                    .default_value(value)
            });
            self.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing || !this.lighting_enabled(region) {
                        return;
                    }
                    let value = match event {
                        SliderEvent::Change(value) => {
                            this.mixer_lighting
                                .preview_values
                                .insert(region, value.start().round() as u32);
                            cx.notify();
                            return;
                        }
                        SliderEvent::Release(value) => value.start().round() as u32,
                    };
                    this.mixer_lighting.preview_values.remove(&region);
                    {
                        if is_fader(region) {
                            let mut speeds = this.draft["device"]["fader"]["dynamicSpeeds"].clone();
                            speeds[(region - 6) as usize]["speed"] = json!(value);
                            this.edit("/device/fader/dynamicSpeeds", speeds, window, cx);
                        } else {
                            this.change_mixer_lighting_brightness(
                                region,
                                None,
                                Some(value),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ));
            self.mixer_lighting.sliders.insert(region, slider);
        }
    }
    pub(super) fn sync_mixer_lighting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 {
            return;
        }
        for region in REGIONS {
            let id = self.lighting_effect_id(region);
            let settings = self.lighting_setting(region);
            if !is_fader(region) || self.draft["device"]["fader"]["useMode"] == 0 {
                self.mixer_lighting
                    .cached
                    .insert((region, id), settings.clone());
            }
            let Some(state) = self.mixer_lighting.regions.get(&region) else {
                continue;
            };
            state.effect.update(cx, |s, cx| {
                s.set_selected_value(&id.to_string(), window, cx)
            });
            for (index, color) in state.colors.iter().enumerate() {
                let value = if let Some(colors) = settings["color1"].as_array() {
                    colors.get(index).and_then(Value::as_str)
                } else {
                    settings[if index == 0 { "color1" } else { "color2" }].as_str()
                };
                let color_value = value
                    .and_then(|s| s.strip_prefix('#'))
                    .filter(|s| s.len() == 6)
                    .and_then(|s| u32::from_str_radix(s, 16).ok());
                color.update(cx, |s, cx| {
                    if let Some(value) = color_value {
                        s.set_value(rgb(value), window, cx)
                    } else {
                        s.clear_value(window, cx)
                    }
                });
            }
        }
        for (region, slider) in &self.mixer_lighting.sliders {
            if self.mixer_lighting.preview_values.contains_key(region) {
                continue;
            }
            let value = if is_fader(*region) {
                self.draft["device"]["fader"]["dynamicSpeeds"][(region - 6) as usize]["speed"]
                    .as_f64()
                    .unwrap_or(2.)
            } else {
                self.draft["profile"]["brightness"][brightness_index(*region)]["value"]
                    .as_f64()
                    .unwrap_or(100.)
            };
            slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
        }
        self.refresh_mixer_lighting_previews();
    }
    fn change_mixer_lighting_effect(
        &mut self,
        region: u32,
        id: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !effect_ids(region).contains(&id) {
            return;
        }
        let setting = self
            .mixer_lighting
            .cached
            .get(&(region, id))
            .cloned()
            .unwrap_or_else(|| default_setting(region, id));
        self.commit_mixer_lighting_effect(region, id, setting, window, cx);
    }
    fn commit_mixer_lighting_effect(
        &mut self,
        region: u32,
        id: u32,
        setting: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mixer_lighting
            .cached
            .insert((region, id), setting.clone());
        self.edit(
            &effect_path(region),
            json!({"regionId":region,"selectedEffectId":id,"selectedEffectSetting":setting}),
            window,
            cx,
        );
    }
    fn change_mixer_lighting_setting(
        &mut self,
        region: u32,
        field: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut setting = self.lighting_setting(region);
        setting[field] = value;
        self.commit_mixer_lighting_effect(
            region,
            self.lighting_effect_id(region),
            setting,
            window,
            cx,
        );
    }
    fn change_mixer_lighting_color(
        &mut self,
        region: u32,
        index: usize,
        color: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if is_fader(region) && self.draft["device"]["fader"]["useMode"] == 1 {
            if color == "no-color" {
                return;
            }
            let mut entries = self.draft["device"]["fader"]["meterEffects"].clone();
            entries[(region - 6) as usize]["selectedEffectSetting"] = json!({"color1":color});
            self.edit("/device/fader/meterEffects", entries, window, cx);
            return;
        }
        let id = self.lighting_effect_id(region);
        let mut settings = self.lighting_setting(region);
        if settings["isRandom"] == true || !matches!(id, 1 | 2) {
            return;
        }
        if id == 1 && color == "no-color" {
            return;
        }
        if let Some(colors) = settings["color1"].as_array_mut() {
            if index >= colors.len() {
                return;
            }
            colors[index] = json!(color);
            if region == 16 && index == 1 {
                for i in [3, 5, 7] {
                    colors[i] = colors[1].clone();
                }
            }
        } else {
            settings[if index == 0 { "color1" } else { "color2" }] = json!(color);
        }
        self.commit_mixer_lighting_effect(region, id, settings, window, cx);
    }
    fn change_mixer_lighting_brightness(
        &mut self,
        region: u32,
        enabled: Option<bool>,
        value: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = &self.draft["profile"]["brightness"][brightness_index(region)];
        let enabled = enabled.unwrap_or(current["isEnabled"].as_bool().unwrap_or(true));
        let value = value.unwrap_or_else(|| {
            if matches!(region, 5 | 33) {
                current["value"].as_u64().unwrap_or(100) as u32
            } else {
                100
            }
        });
        self.edit(
            &format!("/profile/brightness/{}", brightness_index(region)),
            json!({"regionId":region,"isEnabled":enabled,"value":value}),
            window,
            cx,
        );
    }
    fn lighting_color_control(
        &self,
        region: u32,
        index: usize,
        label: String,
        allow_none: bool,
    ) -> AnyElement {
        let state = &self.mixer_lighting.regions[&region];
        v_flex()
            .mt(surface::css(20.))
            .mr(surface::css(20.))
            .child(label.clone())
            .child(
                div().mt(surface::css(5.)).child(
                    LightingColorPicker::new(&state.colors[index], label)
                        .allow_none(allow_none)
                        .disabled(
                            !self.lighting_enabled(region)
                                || self.lighting_setting(region)["isRandom"] == true,
                        ),
                ),
            )
            .into_any_element()
    }
    fn lighting_effect_editor(
        &self,
        region: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = self.lighting_effect_id(region);
        let enabled = self.lighting_enabled(region);
        let setting = self.lighting_setting(region);
        let mut body = v_flex().child(
            Select::new(&self.mixer_lighting.regions[&region].effect)
                .small()
                .h(surface::css(27.))
                .w(surface::css(230.))
                .disabled(!enabled)
                .accessibility_label(t("EFFECT")),
        );
        if id == 1 {
            let count = setting["color1"].as_array().map_or(1, Vec::len);
            body = body.child(
                h_flex()
                    .items_start()
                    .children((0..count.min(8)).map(|index| {
                        self.lighting_color_control(
                            region,
                            index,
                            if count == 4 {
                                t(&format!("CHANNEL_{}_COLOR", index + 1))
                            } else {
                                t("COLOR")
                            },
                            false,
                        )
                    })),
            );
        } else if id == 2 {
            let random = setting["isRandom"] == true;
            body = body.child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .children((0..2).map(|index| {
                        self.lighting_color_control(
                            region,
                            index,
                            t("COLOR_DROP_NAME")
                                .replace("{{num}}", &(index + 1).to_string())
                                .into(),
                            true,
                        )
                    }))
                    .child(
                        div().mt(surface::css(10.)).child(
                            surface::check_item(
                                SharedString::from(format!("mixer-lighting-random-{region}")),
                                t("RANDOM_COLOR"),
                                random,
                                !enabled,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.change_mixer_lighting_setting(
                                        region,
                                        "isRandom",
                                        json!(!random),
                                        window,
                                        cx,
                                    );
                                },
                            )),
                        ),
                    ),
            );
        } else if matches!(id, 4 | 13) {
            let direction = setting["direction"].as_u64().unwrap_or(2);
            body = body.child(
                v_flex().mt(surface::css(20.)).child(t("DIRECTION")).child(
                    h_flex()
                        .mt(surface::css(5.))
                        .children([1u32, 2].map(|next| {
                            BaseButton::new(SharedString::from(format!(
                                "mixer-lighting-direction-{region}-{next}"
                            )))
                            .disabled(!enabled)
                            .w(surface::css(51.))
                            .h(surface::css(27.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(C::border())
                            .bg(if direction == next as u64 {
                                C::selected()
                            } else {
                                C::surface()
                            })
                            .text_color(if direction == next as u64 {
                                C::surface()
                            } else {
                                C::text()
                            })
                            .accessibility_label(if id == 13 {
                                if next == 1 { "Clockwise" } else { "Counterclockwise" }
                            } else {
                                if next == 1 { "Left" } else { "Right" }
                            })
                            .child(
                                img(SharedString::from(format!(
                                    "synapse/audio-mixer-lighting-direction-{}{}.svg",
                                    match (id, next) {
                                        (13, 1) => "cw",
                                        (13, _) => "ccw",
                                        (_, 1) => "left",
                                        _ => "right",
                                    },
                                    if direction == next as u64 { "-active" } else { "" }
                                )))
                                .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.change_mixer_lighting_setting(
                                        region,
                                        "direction",
                                        json!(next),
                                        window,
                                        cx,
                                    )
                                },
                            ))
                        })),
                ),
            );
        }
        body.into_any_element()
    }
    fn lighting_panel(
        &self,
        region: u32,
        title: &str,
        help: &str,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = self.lighting_enabled(region);
        surface::panel_with_title_switch(
            t(title),
            surface::SynapseSwitch::new(SharedString::from(format!(
                "mixer-lighting-switch-{region}"
            )))
            .accessibility_label(t(title))
            .checked(enabled)
            .on_change(cx.listener(move |this, next: &bool, window, cx| {
                if region == 6 {
                    for target in 6..=9 {
                        this.change_mixer_lighting_brightness(
                            target,
                            Some(*next),
                            Some(100),
                            window,
                            cx,
                        );
                    }
                } else {
                    this.change_mixer_lighting_brightness(region, Some(*next), None, window, cx);
                }
            })),
            surface::help_control(
                SharedString::from(format!("mixer-lighting-help-{region}")),
                t(help),
            ),
            cx,
        )
        .child(div().opacity(if enabled { 1. } else { 0.3 }).child(content))
        .into_any_element()
    }
    fn lighting_slider(&self, region: u32) -> AnyElement {
        let slider = &self.mixer_lighting.sliders[&region];
        let value = if let Some(value) = self.mixer_lighting.preview_values.get(&region) {
            f64::from(*value)
        } else if is_fader(region) {
            self.draft["device"]["fader"]["dynamicSpeeds"][(region - 6) as usize]["speed"]
                .as_f64()
                .unwrap_or(2.)
        } else {
            self.draft["profile"]["brightness"][brightness_index(region)]["value"]
                .as_f64()
                .unwrap_or(100.)
        };
        let (min, max) = if is_fader(region) { (1., 3.) } else { (0., 100.) };
        v_flex()
            .child(
                SourceSlider::new(slider, ((value - min) / (max - min)) as f32)
                    .enabled(self.lighting_enabled(region))
                    .range_labels(format!("{min:.0}"), format!("{max:.0}"), C::text())
                    .when(region == 33, |slider| slider.proportional_fill())
                    .tip(Some(format!("{value:.0}"))),
            )
            .into_any_element()
    }
    pub(super) fn dismiss_mixer_lighting(&mut self) {
        self.mixer_lighting.preview_values.clear();
    }
    pub(super) fn mixer_lighting_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let word = v_flex()
            .child(self.mixer_lighting_preview(4))
            .child(div().my(surface::css(16.)).child(t("EFFECT")))
            .child(self.lighting_effect_editor(4, window, cx))
            .into_any_element();
        let base = v_flex()
            .child(self.mixer_lighting_preview(5))
            .child(div().mt(surface::css(20.)).child(t("BASE_BRIGHTNESS")))
            .child(self.lighting_slider(5))
            .child(div().my(surface::css(16.)).child(t("EFFECT")))
            .child(self.lighting_effect_editor(5, window, cx))
            .into_any_element();
        let numbers = v_flex()
            .child(self.mixer_lighting_preview(10))
            .child(div().my(surface::css(16.)).child(t("EFFECT")))
            .child(self.lighting_effect_editor(10, window, cx))
            .into_any_element();
        let mute = self.lighting_mute_controls(16, cx);
        let mic = self.lighting_mute_controls(32, cx);
        let bleep = v_flex()
            .child(self.lighting_mute_controls(33, cx))
            .child(
                div()
                    .mt(surface::css(20.))
                    .child(t("BRIGHTNESS_WHEN_INACTIVE")),
            )
            .child(self.lighting_slider(33))
            .into_any_element();
        let foreground = self.lighting_foreground(window, cx);
        let left = v_flex()
            .child(self.lighting_panel(4, "RAZER_WORDMARK", "RAZER_WORDMARK_TOOLTIP", word, cx))
            .child(self.lighting_panel(5, "FADER_BASE", "FADER_BASE_TOOLTIP", base, cx))
            .child(self.lighting_panel(16, "CHANNEL_MUTE", "CHANNEL_MUTE_TOOLTIP", mute, cx))
            .child(self.lighting_panel(32, "MICROPHONE_MUTE", "MICROPHONE_MUTE_TOOLTIP", mic, cx));
        let right = v_flex()
            .child(self.lighting_panel(10, "CHANNEL_NUMBER", "CHANNEL_NUMBER_TOOLTIP", numbers, cx))
            .child(self.lighting_panel(
                6,
                "FADER_FOREGROUND",
                "FADER_FOREGROUND_TOOLTIP",
                foreground,
                cx,
            ))
            .child(self.lighting_panel(33, "BLEEP_BUTTON", "BLEEP_BUTTON_TOOLTIP", bleep, cx));
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
    fn lighting_mute_controls(&self, region: u32, cx: &App) -> AnyElement {
        let mut body = v_flex();
        if region == 16 {
            body = body
                .child(div().mt(surface::css(20.)).child(t("ACTIVE")))
                .child(
                    h_flex()
                        .justify_between()
                        .children([0usize, 2, 4, 6].map(|index| {
                            v_flex()
                                .child(self.mixer_lighting_button_preview(region, index))
                                .child(self.lighting_color_control(
                                    region,
                                    index,
                                    t("COLOR"),
                                    false,
                                ))
                        })),
                )
                .child(div().mt(surface::css(20.)).child(t("MUTE")))
                .child(
                    v_flex()
                        .child(self.mixer_lighting_button_preview(region, 1))
                        .child(self.lighting_color_control(region, 1, t("COLOR"), false)),
                );
        } else {
            body = body.child(h_flex().children([0usize, 1].map(|index| {
                v_flex()
                    .w_1_2()
                    .child(div().mt(surface::css(20.)).child(t(if region == 33 {
                        if index == 0 { "INACTIVE" } else { "ACTIVE" }
                    } else if index == 0 {
                        "ACTIVE"
                    } else {
                        "MUTE"
                    })))
                    .child(self.mixer_lighting_button_preview(region, index))
                    .child(self.lighting_color_control(region, index, t("COLOR"), false))
            })));
        }
        let _ = cx;
        body.into_any_element()
    }
    fn lighting_foreground(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mode = self.draft["device"]["fader"]["useMode"]
            .as_u64()
            .unwrap_or(0);
        let enabled = self.lighting_enabled(6);
        let mut body = v_flex()
            .child(self.mixer_lighting_preview(6))
            .child(div().mt(surface::css(20.)).child(t("FADER_MODE")))
            .child(
                h_flex().gap(surface::css(20.)).children(
                    ["VOLUME_LEVEL", "INPUT_METER", "DYNAMIC"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, key)| {
                            let owner = cx.entity();
                            Radio::new(SharedString::from(format!("mixer-fader-mode-{index}")))
                                .checked(mode == index as u64)
                                .disabled(!enabled)
                                .accessibility_label(t(key))
                                .child(t(key).to_uppercase())
                                .text_size(surface::css(12.))
                                .px(surface::css(26.))
                                .py(surface::css(7.))
                                .border_1()
                                .border_color(if mode == index as u64 {
                                    C::selected()
                                } else {
                                    C::text()
                                })
                                .rounded(surface::css(5.))
                                .bg(if mode == index as u64 {
                                    C::pressed()
                                } else {
                                    C::surface()
                                })
                                .on_change(move |_, _, window, cx| {
                                    owner.update(cx, |this, cx| {
                                        this.edit("/device/fader/useMode", json!(index), window, cx)
                                    })
                                })
                        }),
                ),
            );
        if mode == 0 {
            for pair in [[6u32, 7], [8, 9]] {
                let mut row = h_flex().gap(surface::css(30.)).items_start();
                for region in pair {
                    row =
                        row.child(
                            v_flex()
                                .flex_1()
                                .child(div().my(surface::css(20.)).child(
                                    t(&format!("CHANNEL_{}_EFFECT", region - 5)).to_uppercase(),
                                ))
                                .child(self.lighting_effect_editor(region, window, cx)),
                        );
                }
                body = body.child(row);
            }
        } else if mode == 1 {
            for pair in [[6u32, 7], [8, 9]] {
                body = body.child(h_flex().children(pair.map(|region| {
                    div().w_1_2().child(self.lighting_color_control(
                        region,
                        0,
                        t(&format!("CHANNEL_{}_COLOR", region - 5)),
                        false,
                    ))
                })));
            }
        } else {
            for pair in [[6u32, 7], [8, 9]] {
                body = body.child(h_flex().gap(surface::css(30.)).children(pair.map(|region| {
                    v_flex()
                        .flex_1()
                        .child(
                            div()
                                .my(surface::css(20.))
                                .child(t(&format!("CHANNEL_{}_SPEED", region - 5)).to_uppercase()),
                        )
                        .child(self.lighting_slider(region))
                })));
            }
        }
        body.into_any_element()
    }
}
