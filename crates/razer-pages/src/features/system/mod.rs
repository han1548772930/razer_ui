//! Native Blade and Book settings from each current product's static source.
//! Profiles, system settings and hardware observations have separate ownership.
//! These controls edit local drafts; they do not fabricate hardware responses.
use gpui_kit::component::{
    ActiveTheme as _, Disableable, Selectable,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    select::{SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_i18n::t;
use razer_widgets::{surface, synapse_select};

use super::controls::Choice;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct SystemProductSpec {
    product_id: u32,
    pages: Vec<String>,
    initial: Value,
    modes: Vec<PerformanceMode>,
    effects: Vec<Effect>,
    logo_effects: Vec<Effect>,
    brightness_step: f32,
    battery_override: bool,
    modern_fans: bool,
    native_display_modes: Vec<Value>,
    speaker_band_start: usize,
    cpu_options: Vec<Value>,
    gpu_options: Vec<Value>,
    mappings: Vec<Value>,
    eq_tabs: Vec<Value>,
    frequencies: Vec<Value>,
}

#[derive(Deserialize)]
struct PerformanceMode {
    mode: String,
    title: String,
    #[serde(default)]
    content: String,
    #[serde(rename = "isShowOnBattery", default)]
    on_battery: bool,
    #[serde(rename = "isAvoidShowOnPlugged", default)]
    battery_only: bool,
    #[serde(rename = "minFanSpeedValue")]
    min_fan: Option<f32>,
    #[serde(rename = "maxFanSpeedValue")]
    max_fan: Option<f32>,
}

#[derive(Deserialize)]
struct Effect {
    id: i64,
    name: String,
}

pub fn source_product(pid: u32) -> Option<&'static SystemProductSpec> {
    static PRODUCTS: super::source_catalog::SourceCatalog<SystemProductSpec> =
        super::source_catalog::SourceCatalog::new(|| read_system_products_catalog());
    PRODUCTS.get(pid)
}

// Every current system build mounts the same brightnessSystemReducer and
// switchOffLightingSystemReducer, each retaining its own activeMode. A tab is
// an editing target, not an AC observation.
#[derive(Clone, Copy, Debug)]
enum PowerSection {
    Performance,
    Brightness,
    SwitchOff,
}

// `checkLowBatteryPercent` slider in every current system build: min 10, max 50, step 1.
const LOW_BATTERY_PERCENT_RANGE: (f32, f32, f32) = (10., 50., 1.);
// Brightness component: the battery warning and slider-tip `*` appear from 80%.
const BRIGHTNESS_BATTERY_WARNING_FROM: f64 = 80.;

// Complete R() switch-off-lighting gate from the current mounted roots.
fn switch_off_disabled(profile: &Value, mode: &str) -> bool {
    let brightness = &profile["brightness"];
    if brightness["isTwoMode"] == true {
        return brightness["twoMode"]["isEnabled"] != true;
    }
    if mode == "twoMode" {
        return brightness["plugged"]["isEnabled"] != true
            && brightness["battery"]["isEnabled"] != true;
    }
    brightness[mode]["isEnabled"] != true
}

fn logo_choices(spec: &SystemProductSpec) -> Vec<Choice> {
    spec.logo_effects
        .iter()
        .map(|effect| Choice::localized(effect.id.to_string(), effect.name.clone()))
        .collect()
}

pub struct SystemProductChanged;

pub struct SystemProductWorkspace {
    spec: &'static SystemProductSpec,
    page: String,
    draft: Value,
    power: String,
    brightness_power: String,
    switch_off_power: String,
    eq_output: String,
    selected_mapping: Option<String>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    ranges: BTreeMap<String, (f32, f32, f32)>,
    // Logo dropdown (`dataSet: LOGO_EFFECTS`, 150px); mounted only when the
    // product has logo effects and `deviceReducer.editionId === 0`.
    logo: Option<Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<SystemProductChanged> for SystemProductWorkspace {}

impl SystemProductWorkspace {
    pub fn new(pid: u32, edition_id: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited system product");
        debug_assert_eq!(spec.product_id, pid, "catalog entry keyed by product id");
        let mut this = Self {
            spec,
            page: "TAB_CUSTOMIZE".into(),
            draft: spec.initial.clone(),
            power: "plugged".into(),
            brightness_power: "plugged".into(),
            switch_off_power: "plugged".into(),
            eq_output: "standard".into(),
            selected_mapping: None,
            sliders: BTreeMap::new(),
            ranges: BTreeMap::new(),
            logo: None,
            subscriptions: vec![],
            syncing: false,
        };
        if !spec.logo_effects.is_empty()
            && edition_id == 0
            && this.draft["profile"]["logoLighting"].is_object()
        {
            this.add_logo_select(window, cx);
        }
        let (battery_min, battery_max, battery_step) = LOW_BATTERY_PERCENT_RANGE;
        for power in ["plugged", "battery", "twoMode"] {
            this.add_slider(
                &format!("/profile/brightness/{power}/value"),
                0.,
                100.,
                spec.brightness_step,
                window,
                cx,
            );
            this.add_slider(
                &format!("/profile/switchOffLighting/{power}/idleMinutes"),
                1.,
                60.,
                1.,
                window,
                cx,
            );
            this.add_slider(
                &format!("/profile/switchOffLighting/{power}/lowBatteryPercent"),
                battery_min,
                battery_max,
                battery_step,
                window,
                cx,
            );
        }
        for mode in &spec.modes {
            for power in ["plugged", "battery"] {
                let root = format!("/performance/{power}/{}", mode.mode);
                if let (Some(min), Some(max)) = (mode.min_fan, mode.max_fan) {
                    let (min, max) = if spec.modern_fans {
                        if power == "battery" {
                            (1000., 3000.)
                        } else {
                            (1000., 5100.)
                        }
                    } else {
                        (min, max)
                    };
                    this.add_slider(
                        &format!("{root}/manualFanSpeed"),
                        min,
                        max,
                        100.,
                        window,
                        cx,
                    );
                }
                for temperature in ["cpu", "gpu"] {
                    let path = format!("{root}/smartFanCurve/{temperature}");
                    let count = this
                        .draft
                        .pointer(&path)
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    for point in 0..count {
                        for fan in ["cpuFanSpeedValue", "gpuFanSpeedValue"] {
                            this.add_slider(
                                &format!("{path}/{point}/{fan}"),
                                1000.,
                                if power == "battery" { 3000. } else { 5100. },
                                100.,
                                window,
                                cx,
                            );
                        }
                    }
                }
            }
        }
        this.add_slider("/battery/value", 50., 80., 5., window, cx);
        for feature in ["volume", "soundNormalization", "voiceClarity"] {
            this.add_slider(
                &format!("/profile/{feature}/value"),
                0.,
                100.,
                1.,
                window,
                cx,
            );
        }
        for output in ["standard", "headphones"] {
            if let Some(presets) = this.draft["eq"][output].as_array().cloned() {
                for (preset_ix, preset) in presets.iter().enumerate() {
                    if let Some(bands) = preset["current"].as_array() {
                        let start = if output == "standard" {
                            spec.speaker_band_start
                        } else {
                            0
                        };
                        for band_ix in start..bands.len() {
                            this.add_slider(
                                &format!("/eq/{output}/{preset_ix}/current/{band_ix}"),
                                -12.,
                                12.,
                                1.,
                                window,
                                cx,
                            );
                        }
                    }
                }
            }
        }
        this.sync_sliders(window, cx);
        this
    }

    pub fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.page = key.into();
            self.selected_mapping = None;
            cx.notify();
        }
    }

    pub fn snapshot(&self) -> Value {
        self.draft.clone()
    }

    pub fn restore(&mut self, saved: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = self.spec.initial.clone();
        if let Some(saved) = saved {
            merge_known(&mut self.draft, saved);
            // An empty default mapping list is an extensible list of overrides,
            // unlike the fixed-length equalizer and fan-curve arrays.
            if let Some(mappings) = saved.pointer("/profile/mappings").and_then(Value::as_array) {
                self.draft["profile"]["mappings"] = Value::Array(
                    mappings
                        .iter()
                        .filter(|mapping| {
                            mapping["outputType"] == "disableGroup"
                                && self
                                    .spec
                                    .mappings
                                    .iter()
                                    .any(|source| source["inputID"] == mapping["inputID"])
                        })
                        .cloned()
                        .collect(),
                );
            }
        }
        for power in ["plugged", "battery"] {
            let current = self.draft["performance"][power]["mode"]
                .as_str()
                .unwrap_or("");
            if !self.spec.modes.iter().any(|m| {
                m.mode == current
                    && if power == "battery" {
                        m.on_battery
                    } else {
                        !m.battery_only
                    }
            }) {
                self.draft["performance"][power]["mode"] =
                    self.spec.initial["performance"][power]["mode"].clone();
            }
            for mode in &self.spec.modes {
                let path = format!("/performance/{power}/{}/active", mode.mode);
                if let Some(active) = self.draft.pointer(&path).and_then(Value::as_str) {
                    let allowed = matches!(active, "auto" | "manual")
                        || (active == "smartFanCurve"
                            && self.spec.modern_fans
                            && power == "plugged");
                    if !allowed {
                        if let Some(initial) = self.spec.initial.pointer(&path) {
                            self.set_value(&path, initial.clone());
                        }
                    }
                }
            }
        }
        if !matches!(self.string("/profile/functionKeyPrimary"), "func" | "multi") {
            self.set_value(
                "/profile/functionKeyPrimary",
                self.spec.initial["profile"]["functionKeyPrimary"].clone(),
            );
        }
        if !self
            .spec
            .effects
            .iter()
            .any(|effect| self.draft["profile"]["quickEffects"]["selectedEffectId"] == effect.id)
        {
            self.draft["profile"]["quickEffects"]["selectedEffectId"] =
                self.spec.initial["profile"]["quickEffects"]["selectedEffectId"].clone();
        }
        for (path, (min, max, step)) in &self.ranges {
            if self.draft.pointer(path) == self.spec.initial.pointer(path) {
                continue;
            }
            if let Some(value) = self.draft.pointer_mut(path) {
                let next = value.as_f64().unwrap_or(*min as f64) as f32;
                *value =
                    json!(((*min + ((next - min) / step).round() * step).clamp(*min, *max)) as f64);
            }
        }
        self.selected_mapping = None;
        self.sync_sliders(window, cx);
        cx.notify();
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
        let key = path.to_owned();
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let next =
                        (min + ((value.start() - min) / step).round() * step).clamp(min, max);
                    this.set_value(&key, json!(next as f64));
                    // Brightness changeValue: `if (e > 0 && !l) setBrightnessEnable(true)`.
                    if let Some(mode) = key
                        .strip_prefix("/profile/brightness/")
                        .and_then(|rest| rest.strip_suffix("/value"))
                    {
                        let enabled = format!("/profile/brightness/{mode}/isEnabled");
                        if next > 0. && !this.checked(&enabled) {
                            this.set_value(&enabled, json!(true));
                        }
                    }
                    this.sync_sliders(window, cx);
                    this.changed(cx);
                }
            },
        ));
        self.ranges.insert(path.into(), (min, max, step));
        self.sliders.insert(path.into(), state);
    }

    fn add_logo_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = logo_choices(self.spec);
        let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
        self.subscriptions.push(
            cx.subscribe_in(&state, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let Ok(id) = id.parse::<i64>() else {
                    return;
                };
                // setLogoSystemEffect writes logoLighting[isTwoMode ? "twoMode" : activeMode].
                let path = format!("/profile/logoLighting/{}/effectId", this.brightness_mode());
                this.set_value(&path, json!(id));
                this.sync_sliders(window, cx);
                this.changed(cx);
            }),
        );
        self.logo = Some(state);
    }

    fn brightness_mode(&self) -> &str {
        if self.checked("/profile/brightness/isTwoMode") {
            "twoMode"
        } else {
            &self.brightness_power
        }
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            }
        }
        if let Some(logo) = &self.logo {
            let path = format!("/profile/logoLighting/{}/effectId", self.brightness_mode());
            let selected = self
                .draft
                .pointer(&path)
                .and_then(Value::as_i64)
                .filter(|id| self.spec.logo_effects.iter().any(|effect| effect.id == *id))
                .map(|id| id.to_string());
            logo.update(cx, |state, cx| match selected {
                Some(id) => state.set_selected_value(&id, window, cx),
                None => state.set_selected_index(None, window, cx),
            });
        }
        self.syncing = false;
    }

    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(SystemProductChanged);
        cx.notify();
    }
    fn checked(&self, path: &str) -> bool {
        self.draft
            .pointer(path)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    fn string(&self, path: &str) -> &str {
        self.draft
            .pointer(path)
            .and_then(Value::as_str)
            .unwrap_or("")
    }
    fn set_value(&mut self, path: &str, value: Value) {
        if let Some(target) = self.draft.pointer_mut(path) {
            *target = value;
        }
    }

    fn toggle(&self, path: &str, label: String, enabled: bool, cx: &Context<Self>) -> AnyElement {
        if !self.draft.pointer(path).is_some_and(Value::is_boolean) {
            return div().into_any_element();
        }
        let path = path.to_owned();
        Checkbox::new(SharedString::from(format!("system-{path}")))
            .label(label)
            .checked(self.checked(&path))
            .disabled(!enabled)
            .on_click(cx.listener(move |this, next, window, cx| {
                if enabled {
                    this.set_value(&path, json!(*next));
                    // Linking brightness changes the logo effect's target mode.
                    this.sync_sliders(window, cx);
                    this.changed(cx);
                }
            }))
            .into_any_element()
    }

    fn choice(
        &self,
        path: &str,
        value: Value,
        label: String,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Button {
        let path = path.to_owned();
        Button::new(SharedString::from(format!("system-{path}-{value}")))
            .outline()
            .label(label)
            .selected(self.draft.pointer(&path) == Some(&value))
            .disabled(!enabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    this.set_value(&path, value.clone());
                    // On battery the source only permits Auto/Fixed fan modes.
                    if path == "/performance/battery/mode" {
                        if let Some(mode) = value.as_str() {
                            let active = format!("/performance/battery/{mode}/active");
                            if this.string(&active) == "smartFanCurve" {
                                this.set_value(&active, json!("auto"));
                            }
                        }
                    }
                    this.sync_sliders(window, cx);
                    this.changed(cx);
                }
            }))
    }

    fn range(&self, path: &str, label: String, unit: &str, enabled: bool) -> AnyElement {
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        let value = self
            .draft
            .pointer(path)
            .and_then(Value::as_f64)
            .unwrap_or_default();
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .child(label)
                    .child(format!("{value:.2}{unit}").replace(".00", "")),
            )
            .child(Slider::new(slider).disabled(!enabled))
            .into_any_element()
    }

    fn selected_power(&self, section: PowerSection) -> &str {
        match section {
            PowerSection::Performance => &self.power,
            PowerSection::Brightness => &self.brightness_power,
            PowerSection::SwitchOff => &self.switch_off_power,
        }
    }

    fn power_tabs(&self, section: PowerSection, cx: &Context<Self>) -> AnyElement {
        // Two-way tab: in two-mode both tabs carry `active`; the left-column
        // `.blade-lighting .is-two-mode .modes-tab` rule disables their pointer events.
        let linked = match section {
            PowerSection::Performance => false,
            PowerSection::Brightness => self.checked("/profile/brightness/isTwoMode"),
            PowerSection::SwitchOff => self.checked("/profile/switchOffLighting/isTwoMode"),
        };
        h_flex()
            .gap_2()
            .children(
                [
                    ("plugged", "LIGHTING_PLUGGED_IN"),
                    ("battery", "LIGHTING_ON_BATTERTY"),
                ]
                .map(|(power, label)| {
                    Button::new(SharedString::from(format!(
                        "system-power-{section:?}-{power}"
                    )))
                    .outline()
                    .label(t(label))
                    .selected(linked || self.selected_power(section) == power)
                    .disabled(linked)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        match section {
                            PowerSection::Performance => this.power = power.into(),
                            PowerSection::Brightness => this.brightness_power = power.into(),
                            PowerSection::SwitchOff => this.switch_off_power = power.into(),
                        }
                        this.sync_sliders(window, cx);
                        cx.notify();
                    }))
                }),
            )
            .into_any_element()
    }

    fn link_toggle(&self, path: &str, cx: &Context<Self>) -> AnyElement {
        // Link icon tip: LIGHTING_LINK_TOOLTIP when unlinked, LIGHTING_UNLINK_TOOLTIP when linked.
        let label = if self.checked(path) {
            "LIGHTING_UNLINK_TOOLTIP"
        } else {
            "LIGHTING_LINK_TOOLTIP"
        };
        self.toggle(path, t(label), true, cx)
    }

    fn performance(&self, cx: &Context<Self>) -> AnyElement {
        let root = format!("/performance/{}", self.power);
        let mode = self.string(&format!("{root}/mode"));
        let on_battery = self.power == "battery";
        let options = self
            .spec
            .modes
            .iter()
            .filter(|m| if on_battery { m.on_battery } else { !m.battery_only });
        let mut modes = v_flex().gap_3();
        for item in options {
            // Some configurations advertise capability-gated modes but do not
            // seed their state. Such modes require a hardware capability reply.
            let enabled = self
                .draft
                .pointer(&format!("{root}/{}", item.mode))
                .is_some();
            modes = modes.child(
                v_flex()
                    .gap_1()
                    .child(self.choice(
                        &format!("{root}/mode"),
                        json!(item.mode),
                        t(&item.title),
                        enabled,
                        cx,
                    ))
                    .child(surface::note(t(&item.content), cx)),
            );
        }
        let mut left = v_flex().child(
            surface::panel(t("PERFORMANCE_MODE_HEADER"), cx)
                .child(self.power_tabs(PowerSection::Performance, cx))
                .child(modes),
        );
        let settings = format!("{root}/{mode}");
        if mode == "custom" {
            let mut custom = surface::panel(t("PERFORMANCE_CUSTOM"), cx);
            for (part, label, options) in [
                ("cpu", "CPU", &self.spec.cpu_options),
                ("gpu", "GPU", &self.spec.gpu_options),
            ] {
                custom =
                    custom.child(
                        v_flex()
                            .gap_2()
                            .child(label)
                            .child(h_flex().flex_wrap().gap_2().children(options.iter().map(
                                |o| {
                                    self.choice(
                                        &format!("{settings}/{part}"),
                                        o["value"].clone(),
                                        t(o["name"].as_str().unwrap_or("")),
                                        !on_battery,
                                        cx,
                                    )
                                },
                            ))),
                    );
            }
            custom = custom.child(self.toggle(
                &format!("{settings}/maxFanSpeedModeEnabled"),
                t("PERFORMANCE_MAXIMUM_FAN_SPEED"),
                !on_battery,
                cx,
            ));
            left = left.child(custom);
        }
        let mut right = v_flex();
        if self.draft.pointer(&format!("{settings}/active")).is_some() {
            let active = self.string(&format!("{settings}/active"));
            let fan_path = format!("{settings}/manualFanSpeed");
            let mut fans = surface::panel(t("PERFORMANCE_FANSPEED"), cx).child(
                h_flex()
                    .gap_2()
                    .child(self.choice(
                        &format!("{settings}/active"),
                        json!("auto"),
                        t("PERFORMANCE_FANSPEED_AUTO"),
                        true,
                        cx,
                    ))
                    .child(self.choice(
                        &format!("{settings}/active"),
                        json!("manual"),
                        t("PERFORMANCE_FANSPEED_MANUAL"),
                        self.sliders.contains_key(&fan_path),
                        cx,
                    )),
            );
            if self.spec.modern_fans
                && self
                    .draft
                    .pointer(&format!("{settings}/smartFanCurve"))
                    .is_some()
            {
                fans = fans.child(self.choice(
                    &format!("{settings}/active"),
                    json!("smartFanCurve"),
                    t("SMART_FAN_CURVE_HEADER"),
                    !on_battery,
                    cx,
                ));
            }
            fans = fans
                .child(self.range(&fan_path, t("FAN_SPEED"), " RPM", active == "manual"))
                .child(self.toggle(
                    &format!("{settings}/cpuBoostEnabled"),
                    t("PERFORMANCE_CPU_BOOST"),
                    on_battery,
                    cx,
                ));
            right = right.child(fans);
            if active == "smartFanCurve" && !on_battery {
                right = right.child(self.fan_curve(&settings, cx));
            }
        }
        if !self.spec.pages.iter().any(|p| p == "TAB_DISPLAY") {
            right = right.child(self.display(cx));
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }

    fn fan_curve(&self, settings: &str, cx: &Context<Self>) -> AnyElement {
        let root = format!("{settings}/smartFanCurve");
        let temperature = self.string(&format!("{root}/activeTemperatureMode"));
        let path = format!("{root}/{temperature}");
        let mut panel = surface::panel(t("SMART_FAN_CURVE_HEADER"), cx)
            .child(h_flex().gap_2().children(["cpu", "gpu"].map(|sensor| {
                self.choice(
                    &format!("{root}/activeTemperatureMode"),
                    json!(sensor),
                    sensor.to_uppercase(),
                    true,
                    cx,
                )
            })))
            .child(self.toggle(
                &format!("{root}/useBothTemperatures"),
                "CPU + GPU".into(),
                true,
                cx,
            ));
        if let Some(points) = self.draft.pointer(&path).and_then(Value::as_array) {
            for (ix, point) in points.iter().enumerate() {
                panel = panel.child(
                    v_flex()
                        .gap_3()
                        .child(format!("{} °C", point["temperature"]))
                        .child(self.range(
                            &format!("{path}/{ix}/cpuFanSpeedValue"),
                            "CPU".into(),
                            " RPM",
                            true,
                        ))
                        .child(self.range(
                            &format!("{path}/{ix}/gpuFanSpeedValue"),
                            "GPU".into(),
                            " RPM",
                            true,
                        )),
                );
            }
        }
        let root = root.to_owned();
        panel
            .child(
                Button::new("system-curve-reset")
                    .outline()
                    .label(t("RESET"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(initial) = this.spec.initial.pointer(&root) {
                            this.set_value(&root, initial.clone());
                        }
                        this.sync_sliders(window, cx);
                        this.changed(cx);
                    })),
            )
            .into_any_element()
    }

    fn battery(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.checked("/battery/isEnabled");
        let override_enabled = self.checked("/battery/batteryChargingOverrideEnabled");
        let mut panel = surface::panel(t("BATTERY_HEALTH_OPTIMIZER"), cx)
            .child(self.toggle(
                "/battery/isEnabled",
                t("BATTERY_HEALTH_OPTIMIZER"),
                true,
                cx,
            ))
            .child(self.range(
                "/battery/value",
                t("BATTERY_HEALTH_OPTIMIZER"),
                "%",
                enabled && !override_enabled,
            ));
        if self.spec.battery_override {
            panel = panel
                .child(self.toggle(
                    "/battery/batteryChargingOverrideEnabled",
                    t("BATTERY_CHARGING_OVERRIDE_CONTENT"),
                    enabled,
                    cx,
                ))
                .child(surface::note(
                    t("BATTERY_CHARGING_OVERRIDE_WARNING_CONTENT"),
                    cx,
                ));
        }
        surface::page_columns()
            .child(surface::page_column(panel))
            .into_any_element()
    }

    fn display(&self, cx: &Context<Self>) -> AnyElement {
        let rates = self.draft["display"]["refreshRateList"].as_array();
        let mut refresh = surface::panel(t("PERFORMANCE_MODE_SCREEN_REFRESH_RATE_HEADER"), cx);
        if let Some(rates) = rates {
            refresh = refresh.child(h_flex().gap_2().children(rates.iter().map(|rate| {
                self.choice(
                    "/display/screenRefreshRate",
                    rate["value"].clone(),
                    rate["name"].as_str().unwrap_or("").into(),
                    rates.len() > 1,
                    cx,
                )
            })));
        }
        // The source replaces the seed list after monitor enumeration. Do not
        // advertise unsupported refresh rates or report a fabricated display.
        refresh = refresh.child(
            Button::new("system-display-settings")
                .outline()
                .label(t("PERFORMANCE_EXTERNAL_DISPLAY_WINDOWS_DISPLAY_SETTINGS"))
                .on_click(|_, _, cx| cx.open_url("ms-settings:display")),
        );
        if self.spec.pages.iter().any(|p| p == "TAB_DISPLAY") {
            refresh = refresh.child(self.toggle(
                "/display/isBatteryModeScreenRefreshRate",
                t("PERFORMANCE_SCREEN_REFRESH_RATE_WARNING"),
                true,
                cx,
            ));
        }
        if !self.spec.native_display_modes.is_empty() {
            refresh = refresh
                .child(div().child(t("NATIVE_DISPLAY_MODE")))
                .child(surface::note(t("NATIVE_DISPLAY_MODE_TITTLE_2"), cx))
                .child(
                    h_flex()
                        .gap_2()
                        .children(self.spec.native_display_modes.iter().map(|mode| {
                            self.choice(
                                "/display/nativeDisplayMode",
                                mode["id"].clone(),
                                format!(
                                    "{} · {} Hz",
                                    mode["resolution"].as_str().unwrap_or(""),
                                    mode["rate"]
                                ),
                                true,
                                cx,
                            )
                        })),
                );
        }
        refresh.into_any_element()
    }

    /// The `displayMode=chromaApp` popup mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn lighting_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.lighting(cx)
    }

    fn lighting(&self, cx: &Context<Self>) -> AnyElement {
        let linked = self.checked("/profile/brightness/isTwoMode");
        let power = self.brightness_mode();
        let brightness = format!("/profile/brightness/{power}");
        let brightness_enabled = self.checked(&format!("{brightness}/isEnabled"));
        let brightness_value = self
            .draft
            .pointer(&format!("{brightness}/value"))
            .and_then(Value::as_f64);
        // A(): enabled && (activeMode !== "plugged" || isTwoMode) && value >= 80.
        let battery_warning = brightness_enabled
            && (linked || power != "plugged")
            && brightness_value.is_some_and(|value| value >= BRIGHTNESS_BATTERY_WARNING_FROM);
        let brightness_panel = surface::panel(t("SYSTEM_LIGHTING"), cx)
            .child(self.power_tabs(PowerSection::Brightness, cx))
            .child(self.link_toggle("/profile/brightness/isTwoMode", cx))
            .child(self.toggle(
                &format!("{brightness}/isEnabled"),
                t("BRIGHTNESS_HEADER"),
                true,
                cx,
            ))
            .child(self.brightness_slider(&brightness, brightness_enabled, battery_warning))
            .when(battery_warning, |panel| {
                panel.child(
                    div()
                        .w_full()
                        .text_right()
                        .text_size(surface::css(15.))
                        .text_color(cx.theme().warning)
                        .child(t("BRIGHTNESS_BATTERY_WARNING")),
                )
            })
            .when_some(self.logo.as_ref(), |panel, logo| {
                panel
                    .child(
                        div()
                            .mt(surface::css(20.))
                            .mb(surface::css(10.))
                            .child(t("LOGO")),
                    )
                    .child(
                        synapse_select::select(logo)
                            .items(logo_choices(self.spec))
                            .accessibility_label(t("LOGO"))
                            .w(surface::css(150.)),
                    )
            });
        let effects =
            surface::panel(t("QUICK_EFFECTS"), cx).child(h_flex().flex_wrap().gap_2().children(
                self.spec.effects.iter().map(|effect| {
                    self.choice(
                        "/profile/quickEffects/selectedEffectId",
                        json!(effect.id),
                        t(&effect.name),
                        true,
                        cx,
                    )
                }),
            ));
        let off_power = if self.checked("/profile/switchOffLighting/isTwoMode") {
            "twoMode"
        } else {
            self.selected_power(PowerSection::SwitchOff)
        };
        let root = format!("/profile/switchOffLighting/{off_power}");
        let off_enabled = !switch_off_disabled(&self.draft["profile"], off_power);
        let battery_label = if off_power == "twoMode" {
            "CHECK_LOW_BATTERY_PERCENT_LINKED"
        } else {
            "CHECK_LOW_BATTERY_PERCENT"
        };
        let off = surface::panel(t("SWITCH_OFF_LIGHTING_HEADER"), cx)
            .child(self.power_tabs(PowerSection::SwitchOff, cx))
            .child(self.link_toggle("/profile/switchOffLighting/isTwoMode", cx))
            .child(self.toggle(
                &format!("{root}/isDisplayOn"),
                t("DISPLAY_TURNED_OFF"),
                off_enabled,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/isIdleEnabled"),
                t("IDLE_FOR_MIN"),
                off_enabled,
                cx,
            ))
            .child(self.range(
                &format!("{root}/idleMinutes"),
                t("IDLE_FOR_MIN"),
                " min",
                off_enabled && self.checked(&format!("{root}/isIdleEnabled")),
            ))
            // M(): activeMode === "battery" || isTwoMode.
            .when(off_power != "plugged", |panel| {
                panel
                    .child(self.toggle(
                        &format!("{root}/isLowBatteryPercent"),
                        t(battery_label),
                        off_enabled,
                        cx,
                    ))
                    .child(self.range(
                        &format!("{root}/lowBatteryPercent"),
                        t(battery_label),
                        "%",
                        off_enabled && self.checked(&format!("{root}/isLowBatteryPercent")),
                    ))
            });
        // `.blade-lighting`: brightness and switch-off share the left column,
        // the effect selector owns the right column.
        surface::page_columns()
            .child(surface::page_column(
                v_flex().child(brightness_panel).child(off),
            ))
            .child(surface::page_column(effects))
            .into_any_element()
    }

    fn brightness_slider(&self, root: &str, enabled: bool, reference: bool) -> AnyElement {
        let path = format!("{root}/value");
        let Some(slider) = self.sliders.get(&path) else {
            return div().into_any_element();
        };
        let value = self
            .draft
            .pointer(&path)
            .and_then(Value::as_f64)
            .unwrap_or_default();
        // `.slider-container.brightness` keeps pointer events while off at
        // opacity .3; the tip gains `*` with the battery warning.
        v_flex()
            .gap_3()
            .when(!enabled, |slider| slider.opacity(0.3))
            .child(
                h_flex()
                    .justify_between()
                    .child(t("OFF"))
                    .child(format!("{value:.0}{}", if reference { "*" } else { "" }))
                    .child(t("BRIGHT")),
            )
            .child(Slider::new(slider))
            .into_any_element()
    }

    fn sound(&self, cx: &Context<Self>) -> AnyElement {
        let mut left = v_flex();
        for (feature, label) in [
            ("volume", "VOLUME"),
            ("soundNormalization", "SOUND_NORMALIZATION_HEADER"),
            ("voiceClarity", "VOICE_CLARITY_HEADER"),
        ] {
            let root = format!("/profile/{feature}");
            if self.draft.pointer(&root).is_some() {
                left = left.child(
                    surface::panel(t(label), cx)
                        .child(self.toggle(&format!("{root}/isEnabled"), t(label), true, cx))
                        .child(self.range(
                            &format!("{root}/value"),
                            t(label),
                            "%",
                            self.checked(&format!("{root}/isEnabled")),
                        )),
                );
            }
        }
        left = left.child(
            surface::panel(t("THX_SPATIAL_AUDIO"), cx)
                .child(self.toggle(
                    "/profile/thxSpatialAudio/isEnabled",
                    t("THX_SPATIAL_AUDIO"),
                    true,
                    cx,
                ))
                .child(
                    Button::new("system-sound-settings")
                        .outline()
                        .label(t("SOUND_PROPERTIES"))
                        .on_click(|_, _, cx| cx.open_url("ms-settings:sound")),
                ),
        );
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(self.equalizer(cx)))
            .into_any_element()
    }

    fn equalizer(&self, cx: &Context<Self>) -> AnyElement {
        let output = &self.eq_output;
        let mut panel = surface::panel(t("EQUALIZER_HEADER"), cx).child(
            h_flex().gap_2().children(
                [
                    ("standard", "SPEAKERS"),
                    ("headphones", "HEADPHONES_HEADER"),
                ]
                .map(|(output, label)| {
                    Button::new(SharedString::from(format!("system-eq-{output}")))
                        .outline()
                        .label(t(label))
                        .selected(self.eq_output == output)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.eq_output = output.into();
                            cx.notify();
                        }))
                }),
            ),
        );
        let selected = self.draft["eq"]["selectedPreset"][output]
            .as_str()
            .unwrap_or("music");
        panel = panel.child(
            h_flex()
                .gap_2()
                .children(self.spec.eq_tabs.iter().filter_map(|preset| {
                    let id = preset["id"].as_str()?;
                    Some(self.choice(
                        &format!("/eq/selectedPreset/{output}"),
                        json!(id),
                        t(preset["name"].as_str().unwrap_or("")),
                        true,
                        cx,
                    ))
                })),
        );
        if let Some(presets) = self.draft["eq"][output].as_array() {
            if let Some((ix, preset)) = presets
                .iter()
                .enumerate()
                .find(|(_, p)| p["name"].as_str() == Some(selected))
            {
                if let Some(bands) = preset["current"].as_array() {
                    let start = if output == "standard" {
                        self.spec.speaker_band_start
                    } else {
                        0
                    };
                    for band_ix in start..bands.len() {
                        let label = self
                            .spec
                            .frequencies
                            .get(band_ix)
                            .map(|v| {
                                v.as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| v.to_string())
                            })
                            .unwrap_or_default();
                        panel = panel.child(self.range(
                            &format!("/eq/{output}/{ix}/current/{band_ix}"),
                            label,
                            " dB",
                            true,
                        ));
                    }
                }
                let path = format!("/eq/{output}/{ix}/current");
                let defaults = preset["default"].clone();
                panel = panel.child(
                    Button::new("system-eq-reset")
                        .outline()
                        .label(t("RESET"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_value(&path, defaults.clone());
                            this.sync_sliders(window, cx);
                            this.changed(cx);
                        })),
                );
            }
        }
        panel.into_any_element()
    }

    /// The `displayMode=armory` root mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.customize(cx)
    }

    fn customize(&self, cx: &Context<Self>) -> AnyElement {
        let root = if self.draft["profile"]["gamingMode"].is_object() {
            "/profile/gamingMode"
        } else {
            "/profile/GamingMode"
        };
        let enabled = self
            .draft
            .pointer(&format!("{root}/state"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            != 0;
        let gaming = surface::panel(t("GAMING_MODE_HEADER"), cx)
            .child(
                h_flex().gap_2().children(
                    [
                        (0, "GAMING_MODE_OFF"),
                        (1, "GAMING_MODE_ON"),
                        (2, "GAMING_MODE_IN_GAME"),
                    ]
                    .map(|(value, label)| {
                        self.choice(&format!("{root}/state"), json!(value), t(label), true, cx)
                    }),
                ),
            )
            .child(self.toggle(
                &format!("{root}/isWindowsKeyDisabled"),
                t("DISABLE_WINDOWS_KEY"),
                enabled,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/isAltTabDisabled"),
                t("DISABLE_ALT_TAB"),
                enabled,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/isAltF4Disabled"),
                t("DISABLE_ALT_F4"),
                enabled,
                cx,
            ));
        let function = surface::panel(t("FUNCTION_KEY_PRIMARY_HEADER"), cx)
            .child(
                h_flex()
                    .gap_2()
                    .child(self.choice(
                        "/profile/functionKeyPrimary",
                        json!("func"),
                        t("FUNCTION_KEYS"),
                        true,
                        cx,
                    ))
                    .child(self.choice(
                        "/profile/functionKeyPrimary",
                        json!("multi"),
                        t("MULTIMEDIA_KEYS"),
                        true,
                        cx,
                    )),
            )
            .child(surface::note(t("FUNCTION_KEY_PRIMARY_DESCRIPTION"), cx));
        let mut keys = surface::panel(t("TAB_CUSTOMIZE"), cx);
        let mut seen = std::collections::BTreeSet::new();
        for mapping in &self.spec.mappings {
            let Some(input) = mapping["inputID"].as_str() else {
                continue;
            };
            if !seen.insert(input) {
                continue;
            }
            let key = input.to_owned();
            keys = keys.child(
                Button::new(SharedString::from(format!("system-key-{input}")))
                    .outline()
                    .label(
                        input
                            .strip_prefix("KEY_")
                            .unwrap_or(input)
                            .replace('_', " "),
                    )
                    .selected(self.selected_mapping.as_deref() == Some(input))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_mapping = Some(key.clone());
                        cx.notify();
                    })),
            );
        }
        if let Some(input) = &self.selected_mapping {
            let key = input.clone();
            keys = keys.child(h_flex().gap_2()
                .child(Button::new("system-key-default").outline().label(t("DEFAULT")).on_click(cx.listener(move |this,_,_,cx| {
                    if let Some(mappings)=this.draft["profile"]["mappings"].as_array_mut(){mappings.retain(|m|m["inputID"].as_str()!=Some(&key));}
                    this.changed(cx);
                })))
                .child(Button::new("system-key-disable").outline().label(t("DISABLE")).on_click({let key=input.clone();cx.listener(move |this,_,_,cx| {
                    if let Some(mappings)=this.draft["profile"]["mappings"].as_array_mut(){
                        mappings.retain(|m|m["inputID"].as_str()!=Some(&key));
                        mappings.push(json!({"inputID":key,"inputType":if key.starts_with("DKM_"){"DKMInput"}else{"KeyInput"},"isHyperShift":true,"outputType":"disableGroup"}));
                    }
                    this.changed(cx);
                })})));
        }
        surface::page_columns()
            .child(surface::page_column(keys))
            .child(surface::page_column(v_flex().child(gaming).child(function)))
            .into_any_element()
    }
}

impl Render for SystemProductWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page.as_str() {
            "TAB_PERFORMANCE" => self.performance(cx),
            "TAB_BATTERY" => self.battery(cx),
            "TAB_DISPLAY" => surface::page_columns()
                .child(surface::page_column(self.display(cx)))
                .into_any_element(),
            "TAB_LIGHTING" => self.lighting(cx),
            "TAB_SOUND" => self.sound(cx),
            "TAB_CUSTOMIZE" => self.customize(cx),
            _ => surface::panel(t(&self.page), cx)
                .child(surface::note("此页尚未完成原生实现。", cx))
                .into_any_element(),
        };
        super::product_surface::body().child(content)
    }
}

fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if matches!(
                    key.as_str(),
                    "temperature"
                        | "default"
                        | "name"
                        | "refreshRateList"
                        | "skuHardwareConfiguration"
                ) {
                    continue;
                }
                if let Some(saved) = saved.get(key) {
                    merge_known(value, saved);
                }
            }
        }
        (Value::Array(target), Value::Array(saved)) => {
            for (value, saved) in target.iter_mut().zip(saved) {
                merge_known(value, saved);
            }
        }
        (target @ Value::Bool(_), Value::Bool(_))
        | (target @ Value::Number(_), Value::Number(_))
        | (target @ Value::String(_), Value::String(_)) => *target = saved.clone(),
        _ => {}
    }
}

#[cfg(test)]
mod lighting_tests {
    use super::{LOW_BATTERY_PERCENT_RANGE, source_product, switch_off_disabled};
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn linked_switch_off_remains_available_when_either_independent_backlight_is_on() {
        for plugged in [false, true] {
            for battery in [false, true] {
                let profile = json!({"brightness":{"isTwoMode":false,
                    "plugged":{"isEnabled":plugged},"battery":{"isEnabled":battery}}});
                assert_eq!(
                    switch_off_disabled(&profile, "twoMode"),
                    !plugged && !battery
                );
                assert_eq!(switch_off_disabled(&profile, "plugged"), !plugged);
                assert_eq!(switch_off_disabled(&profile, "battery"), !battery);
            }
        }
    }

    #[::core::prelude::v1::test]
    fn linked_brightness_controls_both_switch_off_tabs_without_overwriting_them() {
        let profile = json!({"brightness":{"isTwoMode":true,
            "plugged":{"isEnabled":true},"battery":{"isEnabled":true},
            "twoMode":{"isEnabled":false}}});
        for mode in ["plugged", "battery", "twoMode"] {
            assert!(switch_off_disabled(&profile, mode));
        }
    }

    // Every current system build's `checkLowBatteryPercent` slider: min 10, max 50, step 1.
    #[::core::prelude::v1::test]
    fn low_battery_slider_matches_the_shared_switch_off_component() {
        assert_eq!(LOW_BATTERY_PERCENT_RANGE, (10., 50., 1.));
        for pid in [563, 569, 650, 709, 737] {
            assert!(source_product(pid).is_some(), "system product {pid}");
        }
    }
}

// Generated catalog reader: razer-pages/system_products_data.
fn read_system_products_catalog() -> Vec<(u32, fn() -> SystemProductSpec)> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SystemProductSpec> {
        let value = match r.tag()? {
            0 => SystemProductSpec {
                product_id: r.child(decode_1)?,
                pages: r.child(decode_2)?,
                initial: r.child(decode_4)?,
                modes: r.child(decode_5)?,
                effects: r.child(decode_10)?,
                logo_effects: r.child(decode_10)?,
                brightness_step: r.child(decode_9)?,
                battery_override: r.child(decode_7)?,
                modern_fans: r.child(decode_7)?,
                native_display_modes: r.child(decode_13)?,
                speaker_band_start: r.child(decode_14)?,
                cpu_options: r.child(decode_13)?,
                gpu_options: r.child(decode_13)?,
                mappings: r.child(decode_13)?,
                eq_tabs: r.child(decode_13)?,
                frequencies: r.child(decode_13)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<PerformanceMode>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PerformanceMode> {
        let value = match r.tag()? {
            0 => PerformanceMode {
                mode: r.child(decode_3)?,
                title: r.child(decode_3)?,
                content: r.child(decode_3)?,
                on_battery: r.child(decode_7)?,
                battery_only: Default::default(),
                min_fan: r.child(decode_8)?,
                max_fan: r.child(decode_8)?,
            },
            1 => PerformanceMode {
                mode: r.child(decode_3)?,
                title: r.child(decode_3)?,
                content: r.child(decode_3)?,
                on_battery: Default::default(),
                battery_only: Default::default(),
                min_fan: r.child(decode_8)?,
                max_fan: r.child(decode_8)?,
            },
            2 => PerformanceMode {
                mode: r.child(decode_3)?,
                title: r.child(decode_3)?,
                content: r.child(decode_3)?,
                on_battery: Default::default(),
                battery_only: Default::default(),
                min_fan: None,
                max_fan: None,
            },
            3 => PerformanceMode {
                mode: r.child(decode_3)?,
                title: r.child(decode_3)?,
                content: Default::default(),
                on_battery: Default::default(),
                battery_only: Default::default(),
                min_fan: r.child(decode_8)?,
                max_fan: r.child(decode_8)?,
            },
            4 => PerformanceMode {
                mode: r.child(decode_3)?,
                title: r.child(decode_3)?,
                content: r.child(decode_3)?,
                on_battery: r.child(decode_7)?,
                battery_only: r.child(decode_7)?,
                min_fan: r.child(decode_8)?,
                max_fan: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<f32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_9)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Effect>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_11)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Effect> {
        let value = match r.tag()? {
            0 => Effect {
                id: r.child(decode_12)?,
                name: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<i64> {
        let value = match r.tag()? {
            0 => r.i64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn product_563() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            563u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_564() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            564u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_569() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            569u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_570() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            570u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_571() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            571u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_576() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            576u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_581() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            581u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_582() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            582u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_586() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            586u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_587() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            587u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_588() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            588u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_589() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            589u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_594() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            594u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_595() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            595u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_597() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            597u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_598() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            598u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_601() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            601u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_616() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            616u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_618() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            618u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_621() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            621u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_622() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            622u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_623() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            623u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_624() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            624u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_630() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            630u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_633() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            633u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_634() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            634u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_650() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            650u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_651() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            651u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_652() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            652u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_669() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            669u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_670() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            670u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_671() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            671u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_672() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            672u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_694() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            694u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_695() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            695u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_696() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            696u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_709() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            709u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_710() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            710u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_711() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            711u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_736() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            736u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_737() -> SystemProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_SYSTEM_PRODUCTS_DATA,
            737u32,
            "5f8a6ff8f1e65d003fc92a9721133897377cd01ebfca7ec8a0e1117997397eea",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    vec![
        (563u32, product_563 as fn() -> SystemProductSpec),
        (564u32, product_564 as fn() -> SystemProductSpec),
        (569u32, product_569 as fn() -> SystemProductSpec),
        (570u32, product_570 as fn() -> SystemProductSpec),
        (571u32, product_571 as fn() -> SystemProductSpec),
        (576u32, product_576 as fn() -> SystemProductSpec),
        (581u32, product_581 as fn() -> SystemProductSpec),
        (582u32, product_582 as fn() -> SystemProductSpec),
        (586u32, product_586 as fn() -> SystemProductSpec),
        (587u32, product_587 as fn() -> SystemProductSpec),
        (588u32, product_588 as fn() -> SystemProductSpec),
        (589u32, product_589 as fn() -> SystemProductSpec),
        (594u32, product_594 as fn() -> SystemProductSpec),
        (595u32, product_595 as fn() -> SystemProductSpec),
        (597u32, product_597 as fn() -> SystemProductSpec),
        (598u32, product_598 as fn() -> SystemProductSpec),
        (601u32, product_601 as fn() -> SystemProductSpec),
        (616u32, product_616 as fn() -> SystemProductSpec),
        (618u32, product_618 as fn() -> SystemProductSpec),
        (621u32, product_621 as fn() -> SystemProductSpec),
        (622u32, product_622 as fn() -> SystemProductSpec),
        (623u32, product_623 as fn() -> SystemProductSpec),
        (624u32, product_624 as fn() -> SystemProductSpec),
        (630u32, product_630 as fn() -> SystemProductSpec),
        (633u32, product_633 as fn() -> SystemProductSpec),
        (634u32, product_634 as fn() -> SystemProductSpec),
        (650u32, product_650 as fn() -> SystemProductSpec),
        (651u32, product_651 as fn() -> SystemProductSpec),
        (652u32, product_652 as fn() -> SystemProductSpec),
        (669u32, product_669 as fn() -> SystemProductSpec),
        (670u32, product_670 as fn() -> SystemProductSpec),
        (671u32, product_671 as fn() -> SystemProductSpec),
        (672u32, product_672 as fn() -> SystemProductSpec),
        (694u32, product_694 as fn() -> SystemProductSpec),
        (695u32, product_695 as fn() -> SystemProductSpec),
        (696u32, product_696 as fn() -> SystemProductSpec),
        (709u32, product_709 as fn() -> SystemProductSpec),
        (710u32, product_710 as fn() -> SystemProductSpec),
        (711u32, product_711 as fn() -> SystemProductSpec),
        (736u32, product_736 as fn() -> SystemProductSpec),
        (737u32, product_737 as fn() -> SystemProductSpec),
    ]
}

#[cfg(test)]
mod system_products_data_catalog_tests {
    #[test]
    fn packaged_catalog_system_products_data_decodes() {
        for (_, factory) in super::read_system_products_catalog() {
            let _ = factory();
        }
    }
}
// End generated catalog reader: razer-pages/system_products_data.
