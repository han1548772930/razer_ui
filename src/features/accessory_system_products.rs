//! Current Raptor, Hanbo, PWM controller and cooling-pad native local settings.
//! Hardware observations and capability lists are never restored from a profile.
use crate::{
    i18n::{t, t_or},
    ui::surface,
};
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
pub(crate) struct AccessorySystemSpec {
    product_id: u32,
    pages: Vec<String>,
    initial: Value,
    presets: Value,
    enums: Value,
}

pub(crate) fn source_product(pid: u32) -> Option<&'static AccessorySystemSpec> {
    static PRODUCTS: OnceLock<Vec<AccessorySystemSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("accessory_system_products_data.json"))
                .expect("validated accessory system data")
        })
        .iter()
        .find(|p| p.product_id == pid)
}

pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    source_product(pid).is_some_and(|p| p.pages.iter().any(|page| page == key))
}

pub(crate) struct AccessorySystemProductChanged;

pub(crate) struct AccessorySystemProductWorkspace {
    spec: &'static AccessorySystemSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    ranges: BTreeMap<String, (f32, f32, f32)>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    celsius: bool,
    percentage: bool,
}

impl EventEmitter<AccessorySystemProductChanged> for AccessorySystemProductWorkspace {}

impl AccessorySystemProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("current accessory product");
        let mut this = Self {
            spec,
            page: spec.pages[0].clone(),
            draft: spec.initial.clone(),
            sliders: BTreeMap::new(),
            ranges: BTreeMap::new(),
            subscriptions: vec![],
            syncing: false,
            celsius: true,
            percentage: true,
        };
        match pid {
            3858 | 3880 => {
                for field in ["brightness", "contrast"] {
                    this.add_slider(
                        &format!("/gaming/customData/{field}"),
                        0.,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                }
                for color in ["red", "green", "blue"] {
                    this.add_slider(
                        &format!("/color/customData/{color}"),
                        0.,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                }
            }
            3900 => {
                for port in 0..8 {
                    this.add_slider(
                        &format!("/ports/{port}/thermal/fanSpeedValue"),
                        33.,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                    for point in 0..9 {
                        this.add_slider(
                            &format!("/ports/{port}/thermal/advancedSetting/{point}/fanSpeedValue"),
                            25.,
                            100.,
                            1.,
                            window,
                            cx,
                        );
                    }
                }
            }
            3907 => {
                for (mode, min, max) in [
                    ("low", 500., 2000.),
                    ("mid", 1500., 2500.),
                    ("high", 1900., 3200.),
                ] {
                    this.add_slider(
                        &format!("/fanControllerSetting/fixed/{mode}"),
                        min,
                        max,
                        50.,
                        window,
                        cx,
                    );
                }
                for sensor in ["cpu", "gpu", "cpu_gpu"] {
                    for mode in ["quiet", "balanced", "performance"] {
                        let path = format!("/fanControllerSetting/smart/{sensor}/{mode}");
                        let count = this
                            .draft
                            .pointer(&path)
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len);
                        for point in 0..count {
                            this.add_slider(
                                &format!("{path}/{point}/fanSpeedValue"),
                                500.,
                                3200.,
                                1.,
                                window,
                                cx,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        this.sync_sliders(window, cx);
        this
    }

    pub(crate) fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.page = key.into();
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
        }
        // Profile input cannot create hardware data or alter port identity and
        // temperature coordinates. Only validated local choices are admitted.
        for (path, (min, max, step)) in &self.ranges {
            if let Some(value) = self.draft.pointer_mut(path) {
                let next = value.as_f64().unwrap_or(*min as f64) as f32;
                *value = json!((min + ((next - min) / step).round() * step).clamp(*min, *max));
            }
        }
        match self.spec.product_id {
            3858 | 3880 => {
                for (path, name) in [
                    ("/gaming/selectedPreset", "zH"),
                    ("/color/selectedPreset", "GM"),
                    ("/inputSource", "Bi"),
                    ("/secondDisplay/source", "Bi"),
                    ("/refeshRateCounter/position", "e3"),
                    ("/secondDisplay/pipSetting/position", "Ss"),
                ] {
                    let valid = self.spec.enums[name]
                        .as_object()
                        .is_some_and(|o| o.values().any(|v| self.draft.pointer(path) == Some(v)));
                    if !valid {
                        self.reset_value(path);
                    }
                }
                for (path, values) in [
                    ("/secondDisplay/source", vec![15, 17, 19]),
                    ("/refeshRateCounter/position", vec![1, 2, 3, 4]),
                    ("/gaming/customData/overdrive", vec![0, 1, 2]),
                    (
                        "/gaming/customData/gamma",
                        if self.spec.product_id == 3880 {
                            vec![0, 1, 2, 3]
                        } else {
                            vec![0, 1, 2]
                        },
                    ),
                    ("/secondDisplay/mode", vec![1, 2]),
                    ("/secondDisplay/pipSetting/size", vec![1, 2, 3]),
                ] {
                    if !self
                        .draft
                        .pointer(path)
                        .and_then(Value::as_i64)
                        .is_some_and(|v| values.contains(&v))
                    {
                        self.reset_value(path);
                    }
                }
                if self.spec.product_id == 3880
                    && !self.draft["gaming"]["customData"]["gamut"].is_null()
                    && !self.draft["gaming"]["customData"]["gamut"]
                        .as_i64()
                        .is_some_and(|v| [0, 1, 2].contains(&v))
                {
                    self.reset_value("/gaming/customData/gamut");
                }
            }
            3900 => {
                if !self.draft["portsConfig"]["currentActivePort"]
                    .as_i64()
                    .is_some_and(|v| (26..34).contains(&v))
                {
                    self.reset_value("/portsConfig/currentActivePort");
                }
                for ix in 0..8 {
                    let path = format!("/ports/{ix}/thermal/activeThermalIndicator/modeSelected");
                    if !["QUIET", "NORMAL", "PERFORMANCE", "MANUAL", "ADVANCED"]
                        .contains(&self.string(&path))
                    {
                        self.reset_value(&path);
                    }
                    self.normalize_curve(
                        &format!("/ports/{ix}/thermal/advancedSetting"),
                        25.,
                        100.,
                    );
                }
                self.propagate_fans();
            }
            3893 => {
                for path in ["/requestedFanMode", "/requestedPumpMode"] {
                    self.reset_value(path);
                    // Null defaults intentionally accept only these user intents.
                    if let Some(value) = saved.and_then(|s| s.pointer(path)).and_then(Value::as_str)
                    {
                        if ["QUIET", "NORMAL", "PERFORMANCE", "ADVANCED"].contains(&value) {
                            self.set_raw(path, json!(value));
                        }
                    }
                }
            }
            3907 => {
                for (path, values) in [
                    ("/fanControllerSetting/activeMode", vec![1, 2]),
                    ("/fanControllerSetting/fixed/active", vec![0, 1, 2]),
                    ("/fanControllerSetting/smart/active", vec![3, 4, 5]),
                ] {
                    if !self
                        .draft
                        .pointer(path)
                        .and_then(Value::as_i64)
                        .is_some_and(|v| values.contains(&v))
                    {
                        self.reset_value(path);
                    }
                }
                if !["cpu", "gpu", "cpu_gpu"]
                    .contains(&self.string("/fanControllerSetting/smart/activeObject"))
                {
                    self.reset_value("/fanControllerSetting/smart/activeObject");
                }
                for sensor in ["cpu", "gpu", "cpu_gpu"] {
                    for mode in ["quiet", "balanced", "performance"] {
                        self.normalize_curve(
                            &format!("/fanControllerSetting/smart/{sensor}/{mode}"),
                            500.,
                            3200.,
                        );
                    }
                }
            }
            _ => {}
        }
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
                    this.change(&key, json!(next), window, cx);
                }
            },
        ));
        self.ranges.insert(path.into(), (min, max, step));
        self.sliders.insert(path.into(), state);
    }

    fn gaming_value(&self, field: &str) -> &Value {
        let selected = self.draft["gaming"]["selectedPreset"].as_i64().unwrap_or(2);
        if selected == 5 {
            &self.draft["gaming"]["customData"][field]
        } else {
            &self.spec.presets[selected.to_string()][field]
        }
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (path, slider) in &self.sliders {
            let value = if let Some(field) = path.strip_prefix("/gaming/customData/") {
                self.gaming_value(field)
            } else {
                self.draft.pointer(path).unwrap_or(&Value::Null)
            };
            if let Some(value) = value.as_f64() {
                slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        self.syncing = false;
    }

    fn set_raw(&mut self, path: &str, value: Value) {
        if let Some(target) = self.draft.pointer_mut(path) {
            *target = value;
        }
    }
    fn reset_value(&mut self, path: &str) {
        if let Some(value) = self.spec.initial.pointer(path) {
            self.set_raw(path, value.clone());
        }
    }
    fn checked(&self, path: &str) -> bool {
        self.draft
            .pointer(path)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    fn number(&self, path: &str) -> i64 {
        self.draft
            .pointer(path)
            .and_then(Value::as_i64)
            .unwrap_or_default()
    }
    fn string(&self, path: &str) -> &str {
        self.draft
            .pointer(path)
            .and_then(Value::as_str)
            .unwrap_or("")
    }
    fn active_port(&self) -> usize {
        (self.number("/portsConfig/currentActivePort").clamp(26, 33) - 26) as usize
    }

    fn change(
        &mut self,
        path: &str,
        mut value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.allowed(path) {
            return;
        }
        if path.starts_with("/gaming/customData/") && self.draft["gaming"]["selectedPreset"] != 5 {
            let selected = self.number("/gaming/selectedPreset").to_string();
            self.draft["gaming"]["customData"] = self.spec.presets[&selected].clone();
            self.draft["gaming"]["selectedPreset"] = json!(5);
        }
        // Source graphs clamp a node between its neighbours, keeping a
        // nondecreasing thermal response rather than silently moving neighbours.
        if let Some(parent) = path.strip_suffix("/fanSpeedValue") {
            if let Some((curve, index)) = parent.rsplit_once('/') {
                if let Ok(index) = index.parse::<usize>() {
                    if let Some(points) = self.draft.pointer(curve).and_then(Value::as_array) {
                        let low = index
                            .checked_sub(1)
                            .and_then(|i| points.get(i))
                            .and_then(|p| p["fanSpeedValue"].as_f64())
                            .unwrap_or(f64::NEG_INFINITY);
                        let high = points
                            .get(index + 1)
                            .and_then(|p| p["fanSpeedValue"].as_f64())
                            .unwrap_or(f64::INFINITY);
                        if let Some(next) = value.as_f64() {
                            value = json!(next.clamp(low, high));
                        }
                    }
                }
            }
        }
        self.set_raw(path, value);
        if self.spec.product_id == 3900
            && (path.starts_with("/ports/") || path == "/portsConfig/isPortLinked")
        {
            self.propagate_fans();
        }
        self.sync_sliders(window, cx);
        cx.emit(AccessorySystemProductChanged);
        cx.notify();
    }

    fn propagate_fans(&mut self) {
        if !self.checked("/portsConfig/isPortLinked") {
            return;
        }
        let thermal = self.draft["ports"][self.active_port()]["thermal"].clone();
        let mode = thermal["activeThermalIndicator"]["modeSelected"]
            .as_str()
            .unwrap_or("");
        if let Some(ports) = self.draft["ports"].as_array_mut() {
            for port in ports {
                port["thermal"]["activeThermalIndicator"] =
                    thermal["activeThermalIndicator"].clone();
                if mode == "MANUAL" {
                    port["thermal"]["fanSpeedValue"] = thermal["fanSpeedValue"].clone();
                }
                if mode == "ADVANCED" {
                    port["thermal"]["advancedSetting"] = thermal["advancedSetting"].clone();
                }
            }
        }
    }

    fn normalize_curve(&mut self, path: &str, min: f64, max: f64) {
        if let Some(points) = self.draft.pointer_mut(path).and_then(Value::as_array_mut) {
            let mut previous = min;
            for point in points {
                let value = point["fanSpeedValue"]
                    .as_f64()
                    .unwrap_or(previous)
                    .clamp(previous, max);
                point["fanSpeedValue"] = json!(value);
                previous = value;
            }
        }
    }

    fn allowed(&self, path: &str) -> bool {
        if ![3858, 3880].contains(&self.spec.product_id) {
            return true;
        }
        if (path.starts_with("/hdr/") || path.starts_with("/adaptiveSync/"))
            && self.checked("/secondDisplay/isEnabled")
        {
            return false;
        }
        if self.spec.product_id == 3880 {
            if (path.starts_with("/gaming/") || path.starts_with("/color/"))
                && self.checked("/thxCinema/isEnabled")
            {
                return false;
            }
            if path.starts_with("/color/")
                && self.number("/gaming/selectedPreset") == 5
                && self.gaming_value("gamut") != &json!(0)
            {
                return false;
            }
            if path.starts_with("/thxCinema/") && self.number("/color/selectedPreset") == 1 {
                return false;
            }
        }
        true
    }

    fn choice(
        &self,
        path: &str,
        value: Value,
        label: String,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Button {
        let enabled = enabled && self.allowed(path);
        let selected = if let Some(field) = path.strip_prefix("/gaming/customData/") {
            self.gaming_value(field) == &value
        } else {
            self.draft.pointer(path) == Some(&value)
        };
        let path = path.to_owned();
        Button::new(SharedString::from(format!("accessory-{path}-{value}")))
            .outline()
            .label(label)
            .selected(selected)
            .disabled(!enabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    this.change(&path, value.clone(), window, cx);
                }
            }))
    }

    fn toggle(&self, path: &str, label: String, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let enabled = enabled && self.allowed(path);
        let path = path.to_owned();
        Checkbox::new(SharedString::from(format!("accessory-{path}")))
            .label(label)
            .checked(self.checked(&path))
            .disabled(!enabled)
            .on_click(cx.listener(move |this, next, window, cx| {
                if enabled {
                    this.change(&path, json!(*next), window, cx);
                }
            }))
            .into_any_element()
    }

    fn range(&self, path: &str, label: String, unit: &str, enabled: bool) -> AnyElement {
        let enabled = enabled && self.allowed(path);
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        let value = if let Some(field) = path.strip_prefix("/gaming/customData/") {
            self.gaming_value(field)
        } else {
            self.draft.pointer(path).unwrap_or(&Value::Null)
        };
        let value = value.as_f64().unwrap_or_default();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .child(label)
                    .child(format!("{value:.0}{unit}")),
            )
            .child(Slider::new(slider).disabled(!enabled))
            .into_any_element()
    }

    fn positions(&self, path: &str, first: i64, enabled: bool, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap_2()
            .children(
                ["TOPLEFT", "TOPRIGHT", "BOTTOMLEFT", "BOTTOMRIGHT"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        self.choice(path, json!(first + i as i64), t(label), enabled, cx)
                    }),
            )
            .into_any_element()
    }

    fn monitor_gaming(&self, cx: &Context<Self>) -> AnyElement {
        let gamut_locked = self.spec.product_id == 3880 && self.gaming_value("gamut") != &json!(0);
        let mut panel = surface::panel(t("SCARLETT_GAME_MODE_HEADER"), cx)
            .child(
                h_flex().flex_wrap().gap_2().children(
                    [
                        (0, "SCARLETT_DEFAULT"),
                        (1, "FPS"),
                        (3, "MMO"),
                        (2, "RACING"),
                        (4, "STREAMING"),
                        (5, "SCARLETT_CUSTOM"),
                    ]
                    .map(|(id, label)| {
                        self.choice("/gaming/selectedPreset", json!(id), t(label), true, cx)
                    }),
                ),
            )
            .child(self.range(
                "/gaming/customData/brightness",
                t("SCREEN_BRIGHTNESS_HEADER"),
                "",
                true,
            ))
            .child(self.range(
                "/gaming/customData/contrast",
                t("CONTRAST_HEADER"),
                "",
                !gamut_locked,
            ))
            .child(t("OVERDRIVE_HEADER"))
            .child(
                h_flex()
                    .gap_2()
                    .children([(0, "OFF"), (1, "WEAK"), (2, "STRONG")].map(|(id, label)| {
                        self.choice(
                            "/gaming/customData/overdrive",
                            json!(id),
                            t_or(label, label),
                            true,
                            cx,
                        )
                    })),
            )
            .child(t("GAMMA_HEADER"))
            .child(
                h_flex().gap_2().children(
                    ["1.4", "1.8", "2.2", "2.4"]
                        .into_iter()
                        .take(if self.spec.product_id == 3880 { 4 } else { 3 })
                        .enumerate()
                        .map(|(id, label)| {
                            self.choice(
                                "/gaming/customData/gamma",
                                json!(id),
                                label.into(),
                                !gamut_locked,
                                cx,
                            )
                        }),
                ),
            );
        if self.spec.product_id == 3880 {
            panel =
                panel
                    .child(t_or("COLOR_GAMUT", "COLOR GAMUT"))
                    .child(h_flex().gap_2().children(
                        [(0, "NATIVE"), (2, "REC 709"), (1, "DCI-P3")].map(|(id, label)| {
                            self.choice(
                                "/gaming/customData/gamut",
                                json!(id),
                                label.into(),
                                true,
                                cx,
                            )
                        }),
                    ));
        }
        panel.into_any_element()
    }

    fn monitor_color(&self, cx: &Context<Self>) -> AnyElement {
        let mut panel = surface::panel(t("COLOR_TEMPERATURE_HEADER"), cx).child(
            h_flex().flex_wrap().gap_2().children(
                [
                    (5, "NORMAL"),
                    (12, "LOW_BLUE_LIGHT"),
                    (4, "WARM"),
                    (8, "COOL"),
                    (1, "SRGB"),
                    (11, "SCARLETT_CUSTOM"),
                ]
                .map(|(id, label)| {
                    self.choice(
                        "/color/selectedPreset",
                        json!(id),
                        t_or(label, label),
                        true,
                        cx,
                    )
                }),
            ),
        );
        if self.number("/color/selectedPreset") == 11 {
            for (color, label) in [("red", "RED"), ("green", "GREEN"), ("blue", "BLUE")] {
                panel = panel.child(self.range(
                    &format!("/color/customData/{color}"),
                    t_or(label, label),
                    "",
                    true,
                ));
            }
        }
        let mut view = v_flex().gap_5().child(panel);
        if self.spec.product_id == 3880 {
            view = view
                .child(
                    surface::panel(t("THX_CINEMA_HEADER"), cx).child(self.toggle(
                        "/thxCinema/isEnabled",
                        t("THX_CINEMA_HEADER"),
                        true,
                        cx,
                    )),
                )
                .child(
                    surface::panel(t("COLOR_PROFILE_HEADER"), cx)
                        .child(surface::note("等待系统提供显示器色彩配置文件。", cx)),
                )
                .child(surface::panel("HDR", cx).child(self.toggle(
                    "/hdr/isEnabled",
                    "HDR".into(),
                    true,
                    cx,
                )));
        }
        view.into_any_element()
    }

    fn monitor_display(&self, cx: &Context<Self>) -> AnyElement {
        let ports = [(15, "DP_1"), (17, "HDMI_1"), (19, "USB_C")];
        let enabled = self.checked("/secondDisplay/isEnabled");
        let mut pip = surface::panel(t("PIP_HEADER"), cx)
            .child(self.toggle("/secondDisplay/isEnabled", t("PIP_HEADER"), true, cx))
            .child(
                h_flex()
                    .gap_2()
                    .children([(1, "PIP"), (2, "PBP")].map(|(id, label)| {
                        self.choice("/secondDisplay/mode", json!(id), label.into(), enabled, cx)
                    })),
            )
            .child(h_flex().gap_2().children(ports.map(|(id, label)| {
                self.choice(
                    "/secondDisplay/source",
                    json!(id),
                    t_or(label, label),
                    enabled,
                    cx,
                )
            })));
        if self.number("/secondDisplay/mode") == 1 {
            pip =
                pip.child(self.positions("/secondDisplay/pipSetting/position", 0, enabled, cx))
                    .child(h_flex().gap_2().children(
                        [(1, "SMALL"), (2, "MEDIUM"), (3, "LARGE")].map(|(id, label)| {
                            self.choice(
                                "/secondDisplay/pipSetting/size",
                                json!(id),
                                t_or(label, label),
                                enabled,
                                cx,
                            )
                        }),
                    ));
        }
        let mut view = v_flex()
            .gap_5()
            .child(
                surface::panel(t("SCARLETT_INPUT_SOURCE_HEADER"), cx).child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(self.choice("/inputSource", json!(0), t("SCARLETT_AUTO"), true, cx))
                        .children(ports.map(|(id, label)| {
                            self.choice("/inputSource", json!(id), t_or(label, label), true, cx)
                        })),
                ),
            )
            .child(pip)
            .child(surface::panel(t("FREE_SYNC_HEADER"), cx).child(self.toggle(
                "/adaptiveSync/isEnabled",
                t("FREE_SYNC_HEADER"),
                true,
                cx,
            )))
            .child(
                surface::panel(t("FPS_COUNTER_HEADER"), cx)
                    .child(self.toggle(
                        "/refeshRateCounter/isEnabled",
                        t("FPS_COUNTER_HEADER"),
                        true,
                        cx,
                    ))
                    .child(self.positions(
                        "/refeshRateCounter/position",
                        1,
                        self.checked("/refeshRateCounter/isEnabled"),
                        cx,
                    )),
            );
        if self.spec.product_id == 3858 {
            view = view.child(surface::panel("HDR", cx).child(self.toggle(
                "/hdr/isEnabled",
                "HDR".into(),
                true,
                cx,
            )));
        } else {
            view = view.child(
                surface::panel(t_or("REFRESH_RATE", "REFRESH RATE"), cx)
                    .child(surface::note("等待显示器提供支持的刷新率。", cx)),
            );
        }
        view.into_any_element()
    }

    fn units(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .gap_2()
            .children([(true, "°C"), (false, "°F")].map(|(value, label)| {
                Button::new(SharedString::from(format!("accessory-unit-{label}")))
                    .outline()
                    .label(label)
                    .selected(self.celsius == value)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.celsius = value;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    fn curve(
        &self,
        path: &str,
        min: f64,
        max: f64,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let points = self
            .draft
            .pointer(path)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let plotted = points
            .iter()
            .filter_map(|p| Some((p["temperature"].as_f64()?, p["fanSpeedValue"].as_f64()?)))
            .collect::<Vec<_>>();
        let mut view = v_flex()
            .gap_3()
            .child(self.units(cx))
            .child(curve_plot(plotted, min, max, cx));
        for (index, point) in points.iter().enumerate() {
            let temp = point["temperature"].as_f64().unwrap_or_default();
            let temp = if self.celsius { temp } else { temp * 1.8 + 32. };
            let unit = if self.celsius { "°C" } else { "°F" };
            let speed = point["fanSpeedValue"].as_f64().unwrap_or_default();
            let label = if self.spec.product_id == 3907 && self.percentage {
                format!("{temp:.0}{unit} · {:.1}%", speed / 3200. * 100.)
            } else {
                format!("{temp:.0}{unit}")
            };
            view = view.child(self.range(
                &format!("{path}/{index}/fanSpeedValue"),
                label,
                if self.spec.product_id == 3907 {
                    " RPM"
                } else {
                    "%"
                },
                enabled,
            ));
        }
        view.into_any_element()
    }

    fn pwm(&self, cx: &Context<Self>) -> AnyElement {
        let index = self.active_port();
        let root = format!("/ports/{index}/thermal");
        let mode = self.string(&format!("{root}/activeThermalIndicator/modeSelected"));
        let enabled = self.checked(&format!("{root}/activeThermalIndicator/isEnabled"));
        let mut panel = surface::panel(t("FAN_SPEED"), cx)
            .child(h_flex().flex_wrap().gap_2().children((26..34).map(|id| {
                self.choice(
                    "/portsConfig/currentActivePort",
                    json!(id),
                    format!("{} {}", t("FAN"), id - 25),
                    true,
                    cx,
                )
            })))
            .child(self.toggle(
                "/portsConfig/isPortLinked",
                t_or("ALL_FANS", "All fans"),
                true,
                cx,
            ))
            .child(self.toggle(
                &format!("{root}/activeThermalIndicator/isEnabled"),
                t("FAN_SPEED"),
                true,
                cx,
            ))
            .child(h_flex().flex_wrap().gap_2().children(
                ["QUIET", "NORMAL", "PERFORMANCE", "MANUAL", "ADVANCED"].map(|mode| {
                    self.choice(
                        &format!("{root}/activeThermalIndicator/modeSelected"),
                        json!(mode),
                        t_or(mode, mode),
                        enabled,
                        cx,
                    )
                }),
            ));
        if mode == "MANUAL" {
            panel = panel.child(self.range(
                &format!("{root}/fanSpeedValue"),
                t("FAN_SPEED"),
                "%",
                enabled,
            ));
        }
        if mode == "ADVANCED" {
            panel =
                panel.child(self.curve(&format!("{root}/advancedSetting"), 0., 100., enabled, cx));
        }
        panel
            .child(surface::note("转速：— · 等待风扇与外接电源状态。", cx))
            .into_any_element()
    }

    fn hanbo(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .gap_5()
            .children(
                [
                    ("/requestedFanMode", "FAN_SPEED"),
                    ("/requestedPumpMode", "PUMP_SPEED"),
                ]
                .map(|(path, label)| {
                    surface::panel(t(label), cx)
                        .child(h_flex().flex_wrap().gap_2().children(
                            ["QUIET", "NORMAL", "PERFORMANCE", "ADVANCED"].map(|mode| {
                                self.choice(path, json!(mode), t_or(mode, mode), true, cx)
                            }),
                        ))
                        .child(surface::note(
                            "转速：— · 等待设备提供当前模式和温度曲线。",
                            cx,
                        ))
                }),
            )
            .into_any_element()
    }

    fn cooling(&self, cx: &Context<Self>) -> AnyElement {
        let root = "/fanControllerSetting";
        let enabled = self.checked("/fanControllerSetting/isEnabled");
        let smart = self.number("/fanControllerSetting/activeMode") == 2;
        let mut panel = surface::panel(t("FAN_SPEED"), cx)
            .child(self.toggle(&format!("{root}/isEnabled"), t("FAN_SPEED"), true, cx))
            .child(
                h_flex().gap_2().children(
                    [
                        (1, "PERFORMANCE_FANSPEED_FIXED"),
                        (2, "PERFORMANCE_FANSPEED_SMART"),
                    ]
                    .map(|(id, label)| {
                        self.choice(
                            &format!("{root}/activeMode"),
                            json!(id),
                            t_or(label, if id == 1 { "Fixed" } else { "Smart" }),
                            enabled,
                            cx,
                        )
                    }),
                ),
            );
        let reset_path;
        if smart {
            let sensor = self.string("/fanControllerSetting/smart/activeObject");
            let mode = match self.number("/fanControllerSetting/smart/active") {
                3 => "quiet",
                5 => "performance",
                _ => "balanced",
            };
            reset_path = format!("{root}/smart/{sensor}/{mode}");
            panel = panel
                .child(h_flex().gap_2().children(
                    [(3, "QUIET"), (4, "BALANCED"), (5, "PERFORMANCE")].map(|(id, label)| {
                        self.choice(
                            &format!("{root}/smart/active"),
                            json!(id),
                            t_or(label, label),
                            enabled,
                            cx,
                        )
                    }),
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .children([("cpu", "CPU"), ("gpu", "GPU")].map(|(value, label)| {
                            self.choice(
                                &format!("{root}/smart/activeObject"),
                                json!(value),
                                label.into(),
                                enabled,
                                cx,
                            )
                        })),
                )
                .child(
                    Checkbox::new("accessory-percent")
                        .label("%")
                        .checked(self.percentage)
                        .on_click(cx.listener(|this, value, _, cx| {
                            this.percentage = *value;
                            cx.notify();
                        })),
                )
                .child(self.curve(&reset_path, 500., 3200., enabled, cx));
        } else {
            let mode = match self.number("/fanControllerSetting/fixed/active") {
                0 => "low",
                2 => "high",
                _ => "mid",
            };
            reset_path = format!("{root}/fixed/{mode}");
            panel = panel
                .child(
                    h_flex()
                        .gap_2()
                        .children([(0, "LOW"), (1, "MEDIUM"), (2, "HIGH")].map(|(id, label)| {
                            self.choice(
                                &format!("{root}/fixed/active"),
                                json!(id),
                                t_or(label, label),
                                enabled,
                                cx,
                            )
                        })),
                )
                .child(self.range(&reset_path, t("FAN_SPEED"), " RPM", enabled));
        }
        panel
            .child(
                Button::new("accessory-reset-fan")
                    .outline()
                    .label(t("RESET"))
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if enabled {
                            this.reset_value(&reset_path);
                            this.sync_sliders(window, cx);
                            cx.emit(AccessorySystemProductChanged);
                            cx.notify();
                        }
                    })),
            )
            .child(surface::note("CPU：— · GPU：— · 风扇：—", cx))
            .into_any_element()
    }
}

impl Render for AccessorySystemProductWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match (self.spec.product_id, self.page.as_str()) {
            _ if !supports_page(self.spec.product_id, &self.page) => {
                surface::note("?????????????", cx).into_any_element()
            }
            (3858 | 3880, "TAB_GAMING") => self.monitor_gaming(cx),
            (3858 | 3880, "TAB_COLOR") => self.monitor_color(cx),
            (3858 | 3880, "TAB_DISPLAY") => self.monitor_display(cx),
            (3900, "TAB_PERFORMANCE") => self.pwm(cx),
            (3893, "TAB_PERFORMANCE") => self.hanbo(cx),
            (3907, "TAB_PERFORMANCE") => self.cooling(cx),
            _ => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        };
        v_flex()
            .p_5()
            .gap_5()
            .min_w(surface::css(600.))
            .max_w(surface::css(1240.))
            .child(surface::note("本地配置预览；设备状态尚未读取。", cx))
            .child(content)
    }
}

fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if ["id", "name", "temperature"].contains(&key.as_str()) {
                    continue;
                }
                if let Some(next) = saved.get(key) {
                    merge_known(value, next);
                }
            }
        }
        (Value::Array(target), Value::Array(saved)) if target.len() == saved.len() => {
            for (value, next) in target.iter_mut().zip(saved) {
                merge_known(value, next);
            }
        }
        (target @ Value::Bool(_), Value::Bool(value)) => *target = json!(value),
        (target @ Value::Number(_), Value::Number(value)) => *target = json!(value),
        (target @ Value::String(_), Value::String(value)) => *target = json!(value),
        (target @ Value::Null, Value::Number(value)) => *target = json!(value),
        _ => {}
    }
}

fn curve_plot(points: Vec<(f64, f64)>, min: f64, max: f64, cx: &App) -> AnyElement {
    let line = cx.theme().primary;
    let grid = cx.theme().border;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let low = points.first().map_or(0., |p| p.0);
            let high = points.last().map_or(1., |p| p.0).max(low + 1.);
            let pt = |x: f64, y: f64| {
                point(
                    bounds.left() + bounds.size.width * ((x - low) / (high - low)) as f32,
                    bounds.bottom()
                        - bounds.size.height * ((y - min) / (max - min)).clamp(0., 1.) as f32,
                )
            };
            for tick in 0..=4 {
                let y = min + (max - min) * tick as f64 / 4.;
                let mut path = PathBuilder::stroke(px(1.));
                path.move_to(pt(low, y));
                path.line_to(pt(high, y));
                if let Ok(path) = path.build() {
                    window.paint_path(path, grid);
                }
            }
            let mut path = PathBuilder::stroke(px(2.));
            for (index, (x, y)) in points.iter().enumerate() {
                if index == 0 {
                    path.move_to(pt(*x, *y));
                } else {
                    path.line_to(pt(*x, *y));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, line);
            }
        },
    )
    .w_full()
    .h(surface::css(180.))
    .into_any_element()
}
