//! Native Blade and Book settings from each current product's static source.
//! Profiles, system settings and hardware observations have separate ownership.
//! These controls edit local drafts; they do not fabricate hardware responses.
use crate::{i18n::t, ui::surface};
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
pub(crate) struct SystemProductSpec {
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

pub(crate) fn source_product(pid: u32) -> Option<&'static SystemProductSpec> {
    static PRODUCTS: OnceLock<Vec<SystemProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("system_products_data.json"))
                .expect("validated current system specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}

pub(crate) struct SystemProductChanged;

pub(crate) struct SystemProductWorkspace {
    spec: &'static SystemProductSpec,
    page: String,
    draft: Value,
    power: String,
    eq_output: String,
    selected_mapping: Option<String>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    ranges: BTreeMap<String, (f32, f32, f32)>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<SystemProductChanged> for SystemProductWorkspace {}

impl SystemProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited system product");
        let mut this = Self {
            spec,
            page: "TAB_CUSTOMIZE".into(),
            draft: spec.initial.clone(),
            power: "plugged".into(),
            eq_output: "standard".into(),
            selected_mapping: None,
            sliders: BTreeMap::new(),
            ranges: BTreeMap::new(),
            subscriptions: vec![],
            syncing: false,
        };
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
                5.,
                50.,
                5.,
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
        this
    }

    pub(crate) fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.page = key.into();
            self.selected_mapping = None;
            cx.notify();
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
                    this.sync_sliders(window, cx);
                    this.changed(cx);
                }
            },
        ));
        self.ranges.insert(path.into(), (min, max, step));
        self.sliders.insert(path.into(), state);
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            }
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
            .on_click(cx.listener(move |this, next, _, cx| {
                if enabled {
                    this.set_value(&path, json!(*next));
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

    fn power_tabs(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .gap_2()
            .children(
                [
                    ("plugged", "LIGHTING_PLUGGED_IN"),
                    ("battery", "LIGHTING_ON_BATTERTY"),
                ]
                .map(|(power, label)| {
                    Button::new(SharedString::from(format!("system-power-{power}")))
                        .outline()
                        .label(t(label))
                        .selected(self.power == power)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.power = power.into();
                            cx.notify();
                        }))
                }),
            )
            .into_any_element()
    }

    fn performance(&self, cx: &Context<Self>) -> AnyElement {
        let root = format!("/performance/{}", self.power);
        let mode = self.string(&format!("{root}/mode"));
        let on_battery = self.power == "battery";
        let options = self.spec.modes.iter().filter(|m| {
            if on_battery {
                m.on_battery
            } else {
                !m.battery_only
            }
        });
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
        let mut left = v_flex().gap_5().child(
            surface::panel(t("PERFORMANCE_MODE_HEADER"), cx)
                .child(self.power_tabs(cx))
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
        let mut right = v_flex().gap_5();
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
    pub(crate) fn lighting_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.lighting(cx)
    }

    fn lighting(&self, cx: &Context<Self>) -> AnyElement {
        let linked = self.checked("/profile/brightness/isTwoMode");
        let power = if linked { "twoMode" } else { &self.power };
        let brightness = format!("/profile/brightness/{power}");
        let mut left = v_flex().gap_5().child(
            surface::panel(t("BRIGHTNESS_HEADER"), cx)
                .child(self.power_tabs(cx))
                .child(self.toggle(
                    "/profile/brightness/isTwoMode",
                    t("LIGHTING_LINK_TOOLTIP"),
                    true,
                    cx,
                ))
                .child(self.toggle(
                    &format!("{brightness}/isEnabled"),
                    t("BRIGHTNESS"),
                    true,
                    cx,
                ))
                .child(self.range(
                    &format!("{brightness}/value"),
                    t("BRIGHTNESS"),
                    "%",
                    self.checked(&format!("{brightness}/isEnabled")),
                )),
        );
        if self.draft["profile"]["logoLighting"].is_object() {
            left = left.child(
                surface::panel(t("LOGO"), cx).child(h_flex().gap_2().children(
                    self.spec.logo_effects.iter().map(|effect| {
                        self.choice(
                            &format!("/profile/logoLighting/{power}/effectId"),
                            json!(effect.id),
                            t(&effect.name),
                            true,
                            cx,
                        )
                    }),
                )),
            );
        }
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
            &self.power
        };
        let root = format!("/profile/switchOffLighting/{off_power}");
        let off = surface::panel(t("SWITCH_OFF_LIGHTING_HEADER"), cx)
            .child(self.toggle(
                "/profile/switchOffLighting/isTwoMode",
                t("LIGHTING_LINK_TOOLTIP"),
                true,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/isDisplayOn"),
                t("DISPLAY_TURNED_OFF"),
                true,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/isIdleEnabled"),
                t("IDLE_FOR_MIN"),
                true,
                cx,
            ))
            .child(self.range(
                &format!("{root}/idleMinutes"),
                t("IDLE_FOR_MIN"),
                " min",
                self.checked(&format!("{root}/isIdleEnabled")),
            ))
            .child(self.toggle(
                &format!("{root}/isLowBatteryPercent"),
                t("CHECK_LOW_BATTERY_PERCENT"),
                off_power != "plugged",
                cx,
            ))
            .child(self.range(
                &format!("{root}/lowBatteryPercent"),
                t("CHECK_LOW_BATTERY_PERCENT"),
                "%",
                off_power != "plugged" && self.checked(&format!("{root}/isLowBatteryPercent")),
            ));
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(
                v_flex().gap_5().child(effects).child(off),
            ))
            .into_any_element()
    }

    fn sound(&self, cx: &Context<Self>) -> AnyElement {
        let mut left = v_flex().gap_5();
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
    pub(crate) fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
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
            .child(surface::page_column(
                v_flex().gap_5().child(gaming).child(function),
            ))
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
        v_flex()
            .min_w_0()
            .gap_5()
            .py_5()
            .text_color(cx.theme().foreground)
            .child(content)
            .child(surface::note(
                "设置保存在本地配置中；尚未写入电脑硬件。",
                cx,
            ))
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
