//! Native audio controls backed by each product's current mounted source.
//! The snapshot is a local draft: it never claims an audio driver acknowledgement.
use super::Choice;
use crate::{i18n::t, ui::surface};
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[path = "audio_demo.rs"]
mod demo;
#[path = "audio_nommo.rs"]
mod nommo;

#[derive(Deserialize)]
struct AudioOption {
    label: String,
    value: Value,
}
#[derive(Deserialize)]
struct AudioControl {
    path: String,
    label: String,
    kind: String,
    #[serde(default)]
    min: f32,
    #[serde(default)]
    max: f32,
    #[serde(default = "one")]
    step: f32,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    options: Vec<AudioOption>,
    #[serde(default)]
    enabled_by: Option<String>,
    #[serde(default)]
    exclusive_with: Vec<String>,
    #[serde(default)]
    url: Option<String>,
}
fn one() -> f32 {
    1.
}
#[derive(Deserialize)]
struct AudioSection {
    title: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    description: Option<String>,
    controls: Vec<AudioControl>,
    #[serde(default)]
    equalizer: Option<String>,
    #[serde(default)]
    visible_when: Option<AudioCondition>,
}
#[derive(Deserialize)]
struct AudioCondition {
    path: String,
    value: Value,
}
#[derive(Deserialize)]
struct AudioPage {
    key: String,
    sections: Vec<AudioSection>,
}
#[derive(Deserialize)]
struct AudioPreset {
    key: String,
    label: String,
    bands: Vec<f32>,
}
#[derive(Deserialize)]
struct AudioEqualizer {
    key: String,
    frequencies: Vec<String>,
    presets: Vec<AudioPreset>,
    selected: String,
    min: f32,
    max: f32,
    #[serde(default = "one")]
    step: f32,
    #[serde(default)]
    edit_custom: bool,
}
#[derive(Deserialize)]
pub(crate) struct AudioProductSpec {
    product_id: u32,
    draft: Value,
    pages: Vec<AudioPage>,
    #[serde(default)]
    equalizers: Vec<AudioEqualizer>,
}
pub(crate) fn source_product(pid: u32) -> Option<&'static AudioProductSpec> {
    static PRODUCTS: OnceLock<Vec<AudioProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_products_data.json"))
                .expect("validated current audio specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    if key == "TAB_DEMO" && demo::supports(pid) {
        return true;
    }
    source_product(pid).is_some_and(|p| {
        p.pages
            .iter()
            .any(|p| p.key == key && !p.sections.is_empty())
    })
}

#[derive(Deserialize)]
struct ChromaLightingPage {
    product_id: u32,
    page: String,
    body_min_width: f32,
}

/// The current product root must resolve to its own registered lighting
/// component. A generic audio page, or a root name alone, is insufficient.
/// The generated receipt includes the independent `LIGHTING` key of 1465.
fn chroma_lighting_page(pid: u32) -> Option<&'static ChromaLightingPage> {
    static PAGES: OnceLock<Vec<ChromaLightingPage>> = OnceLock::new();
    PAGES
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_chroma_modes.json"))
                .expect("validated audio chroma root-to-page routes")
        })
        .iter()
        .find(|entry| entry.product_id == pid && supports_page(pid, &entry.page))
}

pub(crate) fn supports_chroma_lighting_page(pid: u32) -> bool {
    chroma_lighting_page(pid).is_some()
}
pub(crate) struct AudioProductChanged;
pub(crate) struct AudioProductWorkspace {
    spec: &'static AudioProductSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    selected_region: usize,
    nommo_brightness_dragging: bool,
    demo: Option<Entity<demo::AudioDemo>>,
}
impl EventEmitter<AudioProductChanged> for AudioProductWorkspace {}

impl AudioProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited audio product");
        let mut this = Self {
            spec,
            page: spec.pages.first().map_or(String::new(), |p| p.key.clone()),
            draft: spec.draft.clone(),
            sliders: BTreeMap::new(),
            selects: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
            selected_region: 0,
            nommo_brightness_dragging: false,
            demo: demo::AudioDemo::for_product(pid, cx),
        };
        this.initialize_equalizers();
        for control in spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            if control.kind == "slider" {
                this.add_slider(
                    &control.path,
                    control.min,
                    control.max,
                    control.step,
                    window,
                    cx,
                );
            } else if control.kind == "select" && !this.selects.contains_key(&control.path) {
                // Option identity is its serialized source value, not its translated label.
                let choices = control
                    .options
                    .iter()
                    .map(|o| Choice::new(o.value.to_string(), t(&o.label)))
                    .collect::<Vec<_>>();
                let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
                let path = control.path.clone();
                this.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing {
                            return;
                        }
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Some(value) = this
                                .control(&path)
                                .and_then(|c| {
                                    c.options
                                        .iter()
                                        .find(|o| o.value.to_string() == value.as_str())
                                })
                                .map(|o| o.value.clone())
                            {
                                this.edit(&path, value, window, cx);
                            }
                        }
                    },
                ));
                this.selects.insert(control.path.clone(), state);
            }
        }
        for eq in &spec.equalizers {
            for index in 0..eq.frequencies.len() {
                this.add_slider(
                    &format!("/equalizers/{}/bands/{index}", eq.key),
                    eq.min,
                    eq.max,
                    eq.step,
                    window,
                    cx,
                );
            }
        }
        this.sync(window, cx);
        this
    }
    fn initialize_equalizers(&mut self) {
        self.draft["equalizers"] = json!({});
        for eq in &self.spec.equalizers {
            let selected = eq
                .presets
                .iter()
                .find(|p| p.key == eq.selected)
                .or_else(|| eq.presets.first());
            if let Some(selected) = selected {
                self.draft["equalizers"][&eq.key] = json!({"selected":selected.key, "bands":selected.bands,
                    "presets":eq.presets.iter().map(|p| (p.key.clone(), json!(p.bands))).collect::<serde_json::Map<_,_>>()});
            }
        }
    }
    pub(crate) fn snapshot(&self) -> Value {
        self.draft.clone()
    }
    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = self.spec.draft.clone();
        self.selected_region = 0;
        self.nommo_brightness_dragging = false;
        self.initialize_equalizers();
        if let Some(saved) = saved.filter(|v| v.is_object()) {
            merge_known(&mut self.draft, saved);
        }
        // Restored snapshots never widen a current source's supported range.
        for control in self
            .spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            if control.kind == "slider" {
                if let Some(target) = self.draft.pointer_mut(&control.path) {
                    if let Some(value) = target.as_f64() {
                        *target = normalized(value as f32, control.min, control.max, control.step);
                    }
                }
            } else if control.kind == "select" {
                let valid = self
                    .draft
                    .pointer(&control.path)
                    .is_some_and(|v| control.options.iter().any(|o| o.value == *v));
                if !valid {
                    if let (Some(default), Some(target)) = (
                        self.spec.draft.pointer(&control.path),
                        self.draft.pointer_mut(&control.path),
                    ) {
                        *target = default.clone();
                    }
                }
            }
        }
        for eq in &self.spec.equalizers {
            let state = &mut self.draft["equalizers"][&eq.key];
            if !eq
                .presets
                .iter()
                .any(|p| state["selected"].as_str() == Some(&p.key))
            {
                state["selected"] = json!(eq.selected);
            }
            for field in ["bands"] {
                if let Some(bands) = state[field].as_array_mut() {
                    for value in bands {
                        if let Some(n) = value.as_f64() {
                            *value = normalized(n as f32, eq.min, eq.max, eq.step);
                        }
                    }
                }
            }
            if let Some(presets) = state["presets"].as_object_mut() {
                for bands in presets.values_mut().filter_map(Value::as_array_mut) {
                    for value in bands {
                        if let Some(n) = value.as_f64() {
                            *value = normalized(n as f32, eq.min, eq.max, eq.step);
                        }
                    }
                }
            }
        }
        self.sync(window, cx);
        cx.notify();
    }
    pub(crate) fn set_page(&mut self, page: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            self.page = page.into();
            cx.notify();
        }
    }
    fn control(&self, path: &str) -> Option<&'static AudioControl> {
        self.spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
            .find(|c| c.path == path)
    }
    fn enabled(&self, control: &AudioControl) -> bool {
        self.draft.pointer(&control.path).is_some()
            && control
                .enabled_by
                .as_ref()
                .is_none_or(|p| self.draft.pointer(p).and_then(Value::as_bool) == Some(true))
    }
    fn add_slider(
        &mut self,
        path: &str,
        min: f32,
        max: f32,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sliders.contains_key(path) {
            return;
        }
        let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) else {
            return;
        };
        let state = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(value as f32)
        });
        let path = path.to_owned();
        let key = path.clone();
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if matches!(this.spec.product_id, 1303 | 1304) && key == "/profile/brightness/value"
                {
                    match event {
                        SliderEvent::Change(_) => cx.notify(),
                        SliderEvent::Release(value) => {
                            this.nommo_brightness_dragging = false;
                            this.commit_nommo_brightness(value.start(), window, cx);
                        }
                    }
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    this.edit(&key, normalized(value.start(), min, max, step), window, cx);
                }
            },
        ));
        self.sliders.insert(path, state);
    }
    fn edit(&mut self, path: &str, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.control(path).is_some_and(|c| !self.enabled(c)) {
            return;
        }
        if self.draft.pointer(path) == Some(&value) {
            return;
        }
        let enabled = value == Value::Bool(true);
        if let Some(target) = self.draft.pointer_mut(path) {
            *target = value;
        } else {
            return;
        }
        if enabled {
            if let Some(control) = self.control(path) {
                for peer in &control.exclusive_with {
                    if let Some(target) = self.draft.pointer_mut(peer) {
                        *target = Value::Bool(false);
                    }
                }
            }
        }
        self.sync_haptic_regions(path);
        // Linked stream/playback channel faders are one source-level operation.
        if let Some((channel, rest)) = path
            .rsplit_once("/microphone/")
            .or_else(|| path.rsplit_once("/headphone/"))
        {
            if channel.contains("/otherCategories/")
                && self
                    .draft
                    .pointer(&format!("{channel}/isLinked"))
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                let peer = if path.contains("/microphone/") {
                    "headphone"
                } else {
                    "microphone"
                };
                if let Some(value) = self.draft.pointer(path).cloned() {
                    if let Some(target) =
                        self.draft.pointer_mut(&format!("{channel}/{peer}/{rest}"))
                    {
                        *target = value;
                    }
                }
            }
        }
        if let Some(rest) = path.strip_prefix("/equalizers/") {
            if let Some((key, _)) = rest.split_once("/bands/") {
                if let Some(eq) = self.spec.equalizers.iter().find(|e| e.key == key) {
                    let state = &mut self.draft["equalizers"][key];
                    if eq.edit_custom && eq.presets.iter().any(|p| p.key == "custom") {
                        state["selected"] = json!("custom");
                    }
                    if let Some(selected) = state["selected"].as_str().map(str::to_owned) {
                        state["presets"][selected] = state["bands"].clone();
                    }
                }
            }
        }
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn sync_haptic_regions(&mut self, path: &str) {
        if !path.starts_with("/device/sensa/") {
            return;
        }
        let Some(state) = self.draft.pointer_mut("/device/sensa") else {
            return;
        };
        if let Some(rest) = path.strip_prefix("/device/sensa/regions/") {
            if let Some((index, field)) = rest.split_once('/') {
                if let Ok(index) = index.parse::<usize>() {
                    self.selected_region = index;
                    if state["applyToAllRegions"] == Value::Bool(true) {
                        let value = state["regions"][index][field].clone();
                        if let Some(regions) = state["regions"].as_array_mut() {
                            for region in regions {
                                region[field] = value.clone();
                            }
                        }
                    }
                }
            }
        }
        if path == "/device/sensa/applyToAllRegions"
            && state["applyToAllRegions"] == Value::Bool(true)
        {
            let selected = state["regions"][self.selected_region].clone();
            if let Some(regions) = state["regions"].as_array_mut() {
                for region in regions {
                    region["isEnabled"] = selected["isEnabled"].clone();
                    region["percentage"] = selected["percentage"].clone();
                }
            }
        }
        if path.starts_with("/device/sensa/regions") {
            state["hapticIntensity"]["isEnabled"] = json!(
                state["regions"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|r| r["isEnabled"] == Value::Bool(true)))
            );
        }
        if path == "/device/sensa/hapticIntensity/value"
            && state["hapticIntensity"]["value"].as_f64() == Some(0.)
        {
            state["hapticIntensity"]["isEnabled"] = Value::Bool(false);
        }
        if path == "/device/sensa/hapticIntensity/isEnabled"
            && state["hapticIntensity"]["isEnabled"] == Value::Bool(true)
            && state["hapticIntensity"]["value"].as_f64() == Some(0.)
        {
            state["hapticIntensity"]["value"] =
                self.spec.draft["device"]["sensa"]["hapticIntensity"]["value"].clone();
        }
        if state["hapticIntensity"]["isEnabled"] == Value::Bool(false) {
            if let Some(regions) = state["regions"].as_array_mut() {
                for region in regions {
                    region["isEnabled"] = Value::Bool(false);
                }
            }
        }
    }
    fn select_preset(
        &mut self,
        key: &str,
        preset: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .spec
            .equalizers
            .iter()
            .any(|e| e.key == key && e.presets.iter().any(|p| p.key == preset))
        {
            return;
        }
        let state = &mut self.draft["equalizers"][key];
        state["selected"] = json!(preset);
        state["bands"] = state["presets"][preset].clone();
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn reset_equalizer(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.draft["equalizers"][key];
        if let Some(preset) = self
            .spec
            .equalizers
            .iter()
            .find(|e| e.key == key)
            .and_then(|e| {
                e.presets
                    .iter()
                    .find(|p| Some(p.key.as_str()) == state["selected"].as_str())
            })
        {
            state["bands"] = json!(preset.bands);
            state["presets"][&preset.key] = json!(preset.bands);
            self.sync(window, cx);
            cx.emit(AudioProductChanged);
            cx.notify();
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (path, state) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                state.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        for (path, state) in &self.selects {
            if let Some(value) = self.draft.pointer(path) {
                state.update(cx, |s, cx| {
                    s.set_selected_value(&value.to_string(), window, cx)
                });
            }
        }
        self.syncing = false;
    }
    fn render_control(&self, control: &AudioControl, cx: &mut Context<Self>) -> AnyElement {
        let path = control.path.clone();
        let label = t(&control.label);
        let enabled = self.enabled(control);
        match control.kind.as_str() {
            "link" => gpui_kit::base::Link::new(SharedString::from(format!(
                "audio-link-{path}-{}",
                control.label
            )))
            .href(control.url.clone().unwrap_or_default())
            .accessibility_label(label.clone())
            .open_with(|url, _, _, cx| cx.open_url(url))
            .text_color(cx.theme().primary)
            .child(div().underline().child(label))
            .into_any_element(),
            "unavailable" => Button::new(SharedString::from(format!(
                "audio-unavailable-{path}-{}",
                control.label
            )))
            .small()
            .label(label)
            .disabled(true)
            .into_any_element(),
            "reset" => Button::new(SharedString::from(format!("audio-reset-{path}")))
                .small()
                .ghost()
                .label(label)
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(value) = this.spec.draft.pointer(&path).cloned() {
                        this.edit(&path, value, window, cx);
                    }
                }))
                .into_any_element(),
            "toggle" => Checkbox::new(SharedString::from(format!("audio-{path}")))
                .label(label)
                .checked(
                    self.draft
                        .pointer(&path)
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .disabled(!enabled)
                .on_click(cx.listener(move |this, value, window, cx| {
                    this.edit(&path, json!(value), window, cx)
                }))
                .into_any_element(),
            "select" => v_flex()
                .gap_2()
                .child(label)
                .when_some(self.selects.get(&path), |d, state| {
                    d.child(Select::new(state).disabled(!enabled).w_full())
                })
                .into_any_element(),
            "slider" => {
                let value = self
                    .draft
                    .pointer(&path)
                    .and_then(Value::as_f64)
                    .unwrap_or(f64::from(control.min));
                let digits = if control.step.fract() == 0. {
                    0
                } else {
                    (-control.step.log10()).ceil().clamp(1., 4.) as usize
                };
                let rendered = format!("{value:.digits$}{}", control.unit);
                v_flex()
                    .gap_3()
                    .child(h_flex().justify_between().child(label).child(rendered))
                    .when_some(self.sliders.get(&path), |d, state| {
                        d.child(Slider::new(state).disabled(!enabled))
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}{}", control.min, control.unit))
                            .child(format!("{}{}", control.max, control.unit)),
                    )
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
    fn render_equalizer(&self, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(eq) = self.spec.equalizers.iter().find(|e| e.key == key) else {
            return div().into_any_element();
        };
        let state = &self.draft["equalizers"][key];
        let presets = h_flex()
            .gap_2()
            .flex_wrap()
            .children(eq.presets.iter().map(|preset| {
                let key = key.to_owned();
                let name = preset.key.clone();
                Button::new(SharedString::from(format!("audio-eq-{key}-{name}")))
                    .small()
                    .outline()
                    .label(t(&preset.label))
                    .selected(state["selected"].as_str() == Some(&preset.key))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_preset(&key, &name, window, cx)
                    }))
            }));
        let bands = h_flex()
            .items_start()
            .gap_2()
            .children(eq.frequencies.iter().enumerate().map(|(index, frequency)| {
                let path = format!("/equalizers/{key}/bands/{index}");
                v_flex()
                    .id(SharedString::from(format!("audio-eq-{key}-{frequency}")))
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .child(div().text_xs().child(format!(
                        "{:+.0}",
                        state["bands"][index].as_f64().unwrap_or(0.)
                    )))
                    .when_some(self.sliders.get(&path), |d, s| {
                        d.child(Slider::new(s).vertical().h(surface::css(180.)))
                    })
                    .child(div().text_xs().child(frequency.clone()))
            }));
        let key = key.to_owned();
        v_flex()
            .gap_5()
            .child(presets)
            .child(bands)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} … {:+} dB", eq.min, eq.max)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("audio-eq-reset-{key}")))
                            .small()
                            .ghost()
                            .label(t("RESET"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.reset_equalizer(&key, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}
impl AudioProductWorkspace {
    /// Shared body only: the popup must neither mutate the selected main tab
    /// nor show the local product-name heading. The source root applies
    /// `.body-wrapper{padding:10px 20px 20px}`. Nommo 1303/1304 retain the
    /// body's 600px minimum; their root only unsets `.main-container`.
    pub(crate) fn lighting_element(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let route = chroma_lighting_page(self.spec.product_id)?;
        Some(
            super::product_surface::body()
                .min_w(surface::css(route.body_min_width))
                .child(self.page_body(&route.page, window, cx))
                .into_any_element(),
        )
    }

    fn page_body(&self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page = self.spec.pages.iter().find(|p| p.key == key);
        let mut sections = Vec::new();
        if let Some(page) = page {
            for section in &page.sections {
                if matches!(self.spec.product_id, 1303 | 1304) && key == "TAB_LIGHTING" {
                    if let Some(panel) = self.nommo_lighting_section(section, window, cx) {
                        sections.push(panel);
                        continue;
                    }
                }
                if section
                    .visible_when
                    .as_ref()
                    .is_some_and(|c| self.draft.pointer(&c.path) != Some(&c.value))
                {
                    continue;
                }
                let mut panel =
                    surface::panel(format!("{}{}", t(&section.title), section.suffix), cx).gap_5();
                if let Some(description) = &section.description {
                    panel = panel.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(description)),
                    );
                }
                panel = panel.children(section.controls.iter().map(|c| self.render_control(c, cx)));
                if let Some(key) = &section.equalizer {
                    panel = panel.child(self.render_equalizer(key, cx));
                }
                sections.push(panel.into_any_element());
            }
        }
        if sections.is_empty() {
            sections.push(surface::note("此页面尚未完成。", cx).into_any_element());
        }
        // Current Nommo wm / kM: the left Il / bl column holds brightness
        // followed by switch-off-lighting; the right column holds effects.
        // The former descriptor renderer incorrectly created three columns.
        if matches!(self.spec.product_id, 1303 | 1304)
            && key == "TAB_LIGHTING"
            && sections.len() == 3
        {
            let effects = sections.pop().unwrap();
            return surface::page_columns()
                .child(surface::page_column(v_flex().children(sections)))
                .child(surface::page_column(effects))
                .into_any_element();
        }
        surface::page_columns()
            .children(sections.into_iter().map(surface::page_column))
            .into_any_element()
    }
}
impl Render for AudioProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if matches!(self.spec.product_id, 1303 | 1304) && self.page == "TAB_LIGHTING" {
            if let Some(lighting) = self.lighting_element(window, cx) {
                return lighting;
            }
        }
        if self.page == "TAB_DEMO" {
            if let Some(demo) = &self.demo {
                return demo.clone().into_any_element();
            }
        }
        super::product_surface::body()
            .child(self.page_body(&self.page, window, cx))
            .into_any_element()
    }
}
fn normalized(value: f32, min: f32, max: f32, step: f32) -> Value {
    let value = (min + ((value.clamp(min, max) - min) / step).round() * step).clamp(min, max);
    if step.fract() == 0. {
        json!(value as i64)
    } else {
        json!(value)
    }
}
fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if let Some(saved) = saved.get(key) {
                    merge_known(value, saved);
                }
            }
        }
        (Value::Array(target), Value::Array(saved)) if target.len() == saved.len() => {
            for (target, saved) in target.iter_mut().zip(saved) {
                merge_known(target, saved);
            }
        }
        (target, saved)
            if target.is_boolean() && saved.is_boolean()
                || target.is_number() && saved.is_number()
                || target.is_string() && saved.is_string() =>
        {
            *target = saved.clone()
        }
        _ => {}
    }
}
