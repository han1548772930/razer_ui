//! Current Raptor, Hanbo, PWM controller and cooling-pad native local settings.
//! Hardware observations and capability lists are never restored from a profile.
use crate::{
    backend::system,
    features::Choice,
    i18n::{t, t_or},
    ui::surface,
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

mod corex_fan;

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

/// The monitor HDR widget picks its tip from the host OS version
/// (`t ? r6O.xbh : r6O.UEY` in the current Raptor bundles).
pub(crate) fn hdr_tooltip_key(windows_11: bool) -> &'static str {
    if windows_11 {
        "HDR_TOOLTIP_WINDOWS_11"
    } else {
        "HDR_TOOLTIP"
    }
}

pub(crate) struct AccessorySystemProductChanged;

/// `.btn_group .btn_custom`: the source's flat button — transparent, `1px solid
/// #5d5d5d`, Roboto 12px uppercase, `padding:7px 16px 6px`, `margin:0`,
/// `width:fit-content` — with `.active{background-color:#222;border-color:#44d62c}`
/// and the shared `.btn` `color:#fff`/`:active{opacity:.6}`.
fn source_button(id: impl Into<ElementId>, label: String, active: bool) -> BaseButton {
    BaseButton::new(id)
        .flex()
        .items_center()
        .w_auto()
        .m_0()
        .px(surface::css(16.))
        .pt(surface::css(7.))
        .pb(surface::css(6.))
        .border_1()
        .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
        .bg(if active {
            rgb(0x222222)
        } else {
            rgba(0x00000000)
        })
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .text_color(rgb(0xffffff))
        .active(|style| style.opacity(0.6))
        .child(label.to_uppercase())
}

pub(crate) struct AccessorySystemProductWorkspace {
    spec: &'static AccessorySystemSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    /// `STA`'s `.slider_header input` for the 0–100 rows (`maxStep === 100`):
    /// the source edits the value directly in a 50x27 field.
    inputs: BTreeMap<String, Entity<InputState>>,
    ranges: BTreeMap<String, (f32, f32, f32)>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    celsius: bool,
    percentage: bool,
    /// `PIP_item_hover__fjSri` wrapper state: true while the pointer is over the
    /// PIP picker, which reveals the preset borders and hides the source label.
    pip_hovered: bool,
    /// `C6O` secondary-display source dropdown: the current bundles pass the
    /// `$AA` input list (DP_1 15, HDMI_1 17, USB_C 19).
    pip_source: Entity<SelectState<Vec<Choice>>>,
    /// `RSA` 的 `.screen-refresh-container` 配置文件下拉（`k6O`）。
    color_profile: Entity<SelectState<Vec<Choice>>>,
    /// `SSA` primary-input-source prompt: the source waiting for confirmation.
    /// The source's `shouldAskAgainValue` starts true, so the first change asks.
    pending_input_source: Option<i64>,
    ask_again: bool,
    corex_graph: corex_fan::GraphInteraction,
}

/// `QAA`: the PIP placement presets. Every size is offered in every corner, and
/// the smaller boxes paint above the larger ones (`zIndex` 3/2/1), so the entry
/// carries the source's own z-order.
#[derive(Clone, Copy, PartialEq)]
struct PipSquare {
    size: i64,
    position: i64,
    z: u8,
    width: f32,
    height: f32,
}

const PIP_SQUARES: [PipSquare; 12] = [
    PipSquare {
        size: 1,
        position: 0,
        z: 3,
        width: 90.,
        height: 49.5,
    },
    PipSquare {
        size: 2,
        position: 0,
        z: 2,
        width: 120.,
        height: 66.,
    },
    PipSquare {
        size: 3,
        position: 0,
        z: 1,
        width: 151.,
        height: 83.,
    },
    PipSquare {
        size: 1,
        position: 1,
        z: 3,
        width: 90.,
        height: 49.5,
    },
    PipSquare {
        size: 2,
        position: 1,
        z: 2,
        width: 120.,
        height: 66.,
    },
    PipSquare {
        size: 3,
        position: 1,
        z: 1,
        width: 151.,
        height: 83.,
    },
    PipSquare {
        size: 1,
        position: 2,
        z: 3,
        width: 90.,
        height: 49.5,
    },
    PipSquare {
        size: 2,
        position: 2,
        z: 2,
        width: 120.,
        height: 66.,
    },
    PipSquare {
        size: 3,
        position: 2,
        z: 1,
        width: 151.,
        height: 83.,
    },
    PipSquare {
        size: 1,
        position: 3,
        z: 3,
        width: 90.,
        height: 49.5,
    },
    PipSquare {
        size: 2,
        position: 3,
        z: 2,
        width: 120.,
        height: 66.,
    },
    PipSquare {
        size: 3,
        position: 3,
        z: 1,
        width: 151.,
        height: 83.,
    },
];

/// `$AA`: the secondary input list, in the order the source dropdown shows.
const PIP_SOURCES: [(&str, i64); 3] = [("DP_1", 15), ("HDMI_1", 17), ("USB_C", 19)];

/// `SSA`'s primary input list: the Auto entry (0) followed by HDMI, DisplayPort
/// and USB-C, in the order the widget renders them.
const INPUT_SOURCES: [(i64, &str); 4] = [
    (0, "SCARLETT_AUTO"),
    (17, "HDMI_1"),
    (15, "DP_1"),
    (19, "USB_C"),
];

/// `STA`'s `<input maxLength={3} pattern="(100)|(0*\d{1,2})">`: at most three
/// digits, and never above 100.
pub(crate) fn valid_percent_draft(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && text.len() <= 3
        && text.chars().all(|c| c.is_ascii_digit())
        && text.parse::<u32>().is_ok_and(|value| value <= 100)
}

/// `RTA`'s gamma `maxStep`: 2 on 3858 and 3 on 3880, whose extended slider adds
/// the 2.4 `boostTag` (`useExtendedGammaSlider`).
pub(crate) fn gaming_gamma_steps(pid: u32) -> i64 {
    if pid == 3880 { 3 } else { 2 }
}

/// The gamma row's `minTag`/`midTag`/`maxTag`/`boostTag`.
pub(crate) fn gaming_gamma_tags(
    pid: u32,
) -> (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
) {
    ("1.4", "1.8", "2.2", (pid == 3880).then_some("2.4"))
}

/// The overdrive row's `minTag`/`midTag`/`maxTag`: `maxStep` 2 with no boost tag.
pub(crate) fn gaming_overdrive_tags() -> (&'static str, &'static str, &'static str) {
    ("OFF", "WEAK", "STRONG")
}

/// `GM`：色彩预设枚举（`NORMAL:5,LOWBLUELIGHT:12,WARM:4,COOL:8,SRGB:1,CUSTOM:11`），
/// 顺序即 `Object.entries(GM)` 的渲染顺序，标签键是 `KAA["SCARLETT_"+name]` 解析出的
/// 基础键。
pub(crate) const COLOR_PRESETS: [(i64, &str); 6] = [
    (5, "NORMAL"),
    (12, "LOW_BLUE_LIGHT"),
    (4, "WARM"),
    (8, "COOL"),
    (1, "SRGB"),
    (11, "SCARLETT_CUSTOM"),
];

/// `NSA` 把四个显示器色彩组件放进两个 `.widget-col{width:600px}` 列：
/// 左列 THX Cinema、Color Profile，右列 HDR、Color Temperature。
pub(crate) const COLOR_PAGE_COLUMNS: [[&str; 2]; 2] = [
    [
        "THX_CINEMA_HEADER",
        "PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER",
    ],
    ["HDR_HEADER", "COLOR_TEMPERATURE_HEADER"],
];

/// What a requested primary input source does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputSourceStep {
    /// `confirmInputSourceChange` ignores the source already in use.
    Ignore,
    /// The prompt is enabled, so the change waits for `OSA`.
    Prompt,
    /// The prompt was dismissed with "don't ask me again".
    Apply,
}

pub(crate) fn input_source_step(current: i64, requested: i64, ask_again: bool) -> InputSourceStep {
    if current == requested {
        InputSourceStep::Ignore
    } else if ask_again {
        InputSourceStep::Prompt
    } else {
        InputSourceStep::Apply
    }
}

impl EventEmitter<AccessorySystemProductChanged> for AccessorySystemProductWorkspace {}

impl AccessorySystemProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("current accessory product");
        let choices = PIP_SOURCES
            .iter()
            .map(|(key, id)| Choice::new(id.to_string(), t(key)))
            .collect::<Vec<_>>();
        let pip_source = cx.new(|cx| SelectState::new(choices, None, window, cx));
        // `RSA` 的配置文件下拉：原版数据来自扩展上报的 `colorProfiles`，本地没有该
        // 数据源，因此走原版自己的兜底分支——一条空选项，且 `z6O` 在选项 ≤1 时禁用。
        let color_profile =
            cx.new(|cx| SelectState::new(vec![Choice::new("", "")], None, window, cx));
        let mut this = Self {
            spec,
            page: spec.pages[0].clone(),
            draft: spec.initial.clone(),
            sliders: BTreeMap::new(),
            inputs: BTreeMap::new(),
            ranges: BTreeMap::new(),
            subscriptions: vec![],
            syncing: false,
            celsius: true,
            percentage: true,
            pip_hovered: false,
            pip_source: pip_source.clone(),
            color_profile,
            pending_input_source: None,
            ask_again: true,
            corex_graph: corex_fan::GraphInteraction::new(cx),
        };
        this.subscriptions.push(cx.subscribe_in(
            &pip_source,
            window,
            |this, state, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event
                    && let Ok(id) = value.parse::<i64>()
                    && PIP_SOURCES.iter().any(|(_, source)| *source == id)
                {
                    this.change("/secondDisplay/source", json!(id), window, cx);
                }
                let _ = state;
            },
        ));
        this.sync_pip_source(window, cx);
        match pid {
            3858 | 3880 => {
                for field in ["brightness", "contrast"] {
                    let path = format!("/gaming/customData/{field}");
                    this.add_slider(&path, 0., 100., 1., window, cx);
                    // `STA` shows the numeric field only when `maxStep === 100`.
                    this.add_percent_input(&path, window, cx);
                }
                // Overdrive and gamma are discrete sliders in the source, not
                // button rows: `maxStep` 2 (0–2) and 2 or 3 (0–2 / 0–3 on 3880).
                this.add_slider("/gaming/customData/overdrive", 0., 2., 1., window, cx);
                this.add_slider(
                    "/gaming/customData/gamma",
                    0.,
                    gaming_gamma_steps(pid) as f32,
                    1.,
                    window,
                    cx,
                );
                for color in ["red", "green", "blue"] {
                    this.add_slider(
                        &format!("/color/customData/{color}"),
                        0.,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                    // `STA` 的数值输入框只在 `maxStep===100` 时出现。
                    this.add_percent_input(&format!("/color/customData/{color}"), window, cx);
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
        if pid == 3921 {
            this.init_corex(window, cx);
        }
        this.sync_sliders(window, cx);
        this
    }

    pub(crate) fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.corex_graph.dragging = false;
            // The source's prompt lives in the page component, so leaving the page
            // drops a request that was still waiting for confirmation.
            self.pending_input_source = None;
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
            if self.spec.product_id == 3921 {
                self.restore_corex(saved);
            }
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

    /// `STA`'s numeric field for the 0–100 rows: the source renders a plain
    /// `<input maxLength=3 pattern="(100)|(0*\d{1,2})">` and commits the parsed
    /// value, so the retained editor accepts the same drafts and writes on
    /// Enter/blur like the other source number fields.
    fn add_percent_input(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(value) = self
            .draft
            .pointer(path)
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.0}"))
        else {
            return;
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|text, _| valid_percent_draft(text))
                .default_value(value)
        });
        let key = path.to_owned();
        self.subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, state, event, window, cx| {
                if this.syncing {
                    return;
                }
                if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    return;
                }
                let text = state.read(cx).value().to_string();
                let Some(next) = text
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| (0.0..=100.0).contains(v))
                else {
                    this.sync_sliders(window, cx);
                    return;
                };
                if this.number(&key) != next.round() as i64 {
                    this.change(&key, json!(next.round() as i64), window, cx);
                }
            },
        ));
        self.inputs.insert(path.into(), input);
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        if self.spec.product_id == 3921 {
            self.sync_corex(window, cx);
        }
        for (path, slider) in &self.sliders {
            let value = if let Some(field) = path.strip_prefix("/gaming/customData/") {
                self.gaming_value(field)
            } else {
                self.draft.pointer(path).unwrap_or(&Value::Null)
            };
            if let Some(value) = value.as_f64() {
                slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
            if let Some(input) = self.inputs.get(path) {
                let text = value
                    .as_f64()
                    .map(|value| format!("{value:.0}"))
                    .unwrap_or_default();
                if input.read(cx).value().as_str() != text {
                    input.update(cx, |state, cx| state.set_value(text, window, cx));
                }
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

    /// The dropdown mirrors the requested secondary source, which is local
    /// profile intent; it never claims to be observed device state.
    fn sync_pip_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.number("/secondDisplay/source");
        let Some((_, id)) = PIP_SOURCES.iter().find(|(_, id)| *id == current) else {
            return;
        };
        self.pip_source.update(cx, |state, cx| {
            state.set_selected_value(&id.to_string(), window, cx);
        });
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
        if self.spec.product_id == 3921 {
            self.corex_graph.clear_selection();
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
        if path == "/secondDisplay/source" {
            self.sync_pip_source(window, cx);
        }
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

    /// `.widget .help`: a 14px control carrying the shared question-mark glyph
    /// (`tooltip_questionmark.96138d2f.svg`, byte-identical in every fetched
    /// bundle), `background-color:#4a4a4a` rising to `#ffffff4d` over `.3s`,
    /// placed in the widget's title row. Hovering it reveals `.widget .tip`,
    /// which `SourceTooltip` renders with the audited 14px/18px 300px cap.
    /// `.widget .help`（见 `surface::help_control`）：原版组件标题右上角的
    /// 14px 帮助控件，悬停显示 `.widget .tip`。
    fn help_control(
        &self,
        id: impl Into<ElementId>,
        text: String,
        cx: &Context<Self>,
    ) -> AnyElement {
        let _ = cx;
        surface::help_control(id, text)
    }

    /// `NSA`: the HDR widget's tips follow the host OS: Windows 11 names
    /// "Brightness & Color", earlier Windows names "Windows HD Color".
    fn hdr_tip(&self) -> String {
        t(hdr_tooltip_key(system::is_windows_11()))
    }

    /// `qAA` / `ZAA` screen mock: a 330x196 display with a 186px main area (the
    /// positioning context for its content), a 10px `#5d5d5d` footer bar with
    /// 2px bottom corners and the 130x30 `#222` stand below it.
    fn screen_mock(&self, content: AnyElement) -> AnyElement {
        v_flex()
            .items_center()
            .child(
                v_flex()
                    .w(surface::css(330.))
                    .h(surface::css(196.))
                    .rounded(surface::css(5.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(surface::css(186.))
                            .child(content),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(surface::css(10.))
                            .bg(rgb(0x5d5d5d))
                            .rounded_b(surface::css(2.)),
                    ),
            )
            .child(
                div()
                    .w(surface::css(130.))
                    .h(surface::css(30.))
                    .mt(surface::css(2.))
                    .bg(rgb(0x222222)),
            )
            .into_any_element()
    }

    /// `JAA` / `PIP_wrapper__cCZfo`: the twelve placement presets over the screen
    /// mock. `.PIP_item` boxes are `1px dashed #cccccc1a`; a hovered item paints
    /// `#ffffff1a` with a `#44d62c` border. While the pointer is anywhere over the
    /// picker the wrapper's `PIP_item_hover` state turns every item border `#ccc`,
    /// the hovered item and the `PIP_source__3fPv8` box `#44d62c`, and hides the
    /// source label. Presets paint in the source's z order (3/2/1, smaller above).
    fn pip_picker(&self, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let size = self.number("/secondDisplay/pipSetting/size");
        let position = self.number("/secondDisplay/pipSetting/position");
        let selected = PIP_SQUARES
            .iter()
            .find(|square| square.size == size && square.position == position)
            .unwrap_or(&PIP_SQUARES[0]);
        let label = PIP_SOURCES
            .iter()
            .find(|(_, id)| *id == self.number("/secondDisplay/source"))
            .map_or(String::new(), |(key, _)| t(key));
        let pointer = self.pip_hovered;
        let mut wrapper = div()
            .id("accessory-pip-picker")
            .relative()
            .size_full()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if this.pip_hovered != *hovered {
                    this.pip_hovered = *hovered;
                    cx.notify();
                }
            }));
        for z in [1, 2, 3] {
            if selected.z == z {
                let mut box_view = div()
                    .absolute()
                    .w(surface::css(selected.width))
                    .h(surface::css(selected.height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x222222))
                    .border_1()
                    .border_color(if pointer {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x5d5d5d)
                    })
                    .when(!pointer, |view| {
                        view.child(
                            div()
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .text_color(rgb(0x999999))
                                .child(label.clone()),
                        )
                    });
                box_view = if selected.position % 2 == 0 {
                    box_view.left(px(0.))
                } else {
                    box_view.right(px(0.))
                };
                box_view = if selected.position < 2 {
                    box_view.top(px(0.))
                } else {
                    box_view.bottom(px(0.))
                };
                wrapper = wrapper.child(box_view);
            }
            wrapper = wrapper.children(
                PIP_SQUARES
                    .iter()
                    .filter(|square| square.z == z && *square != selected)
                    .map(|square| {
                        let id = (square.size, square.position);
                        let mut item = div()
                            .absolute()
                            .w(surface::css(square.width))
                            .h(surface::css(square.height))
                            .border_1()
                            .border_dashed()
                            .border_color(rgb(0xcccccc1a))
                            .when(pointer, |item| item.border_color(rgb(0xcccccc)))
                            .hover(|style| style.bg(rgba(0xffffff1a)).border_color(rgb(0x44d62c)))
                            .when(enabled, |item| {
                                item.on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.set_pip_setting(id.0, id.1, window, cx);
                                    }),
                                )
                            });
                        item = if square.position % 2 == 0 {
                            item.left(px(0.))
                        } else {
                            item.right(px(0.))
                        };
                        item = if square.position < 2 {
                            item.top(px(0.))
                        } else {
                            item.bottom(px(0.))
                        };
                        item
                    }),
            );
        }
        wrapper.into_any_element()
    }

    fn set_pip_setting(
        &mut self,
        size: i64,
        position: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.number("/secondDisplay/pipSetting/size") != size {
            self.change("/secondDisplay/pipSetting/size", json!(size), window, cx);
        }
        if self.number("/secondDisplay/pipSetting/position") != position {
            self.change(
                "/secondDisplay/pipSetting/position",
                json!(position),
                window,
                cx,
            );
        }
    }

    /// `eSA`: the PIP widget body. The screen mock (left) hosts the picker in PIP
    /// mode and the `PIPContainer_source__cAUwV` box in PBP mode; the right column
    /// carries the `MODE` heading, the two 144x80 mode buttons with their miniature
    /// display graphics, the `SCARLETT_SOURCE` heading and the source dropdown.
    fn pip_display(&self, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let mode = self.number("/secondDisplay/mode");
        let source = PIP_SOURCES
            .iter()
            .find(|(_, id)| *id == self.number("/secondDisplay/source"))
            .map_or(String::new(), |(key, _)| t(key));
        let screen = if mode == 2 {
            self.screen_mock(
                div()
                    .absolute()
                    .right(px(0.))
                    .top(px(0.))
                    .h_full()
                    .w(surface::css(165.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x222222))
                    .border_l_1()
                    .border_color(rgb(0x5d5d5d))
                    .child(
                        div()
                            .font_family("Roboto")
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(rgb(0x999999))
                            .child(source),
                    )
                    .into_any_element(),
            )
        } else {
            self.screen_mock(self.pip_picker(enabled, cx))
        };
        // `.featureDisabled{opacity:.3;pointer-events:none}` wraps the whole body
        // when the second display is off; the widget's own switch stays outside.
        h_flex()
            .items_start()
            .when(!enabled, |row| row.opacity(0.3))
            .child(screen)
            .child(
                v_flex()
                    .ml(surface::css(20.))
                    .child(
                        div()
                            .mb(surface::css(8.))
                            .font_family("Roboto")
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(rgb(0xcccccc))
                            .child(t("MODE")),
                    )
                    .child(
                        // `.action_button_group`: a 290px column with `mb20`.
                        v_flex()
                            .w(surface::css(290.))
                            .mb(surface::css(20.))
                            .child(self.pip_mode_button(mode == 1, true, enabled, cx))
                            .child(div().mt(surface::css(10.)).child(self.pip_mode_button(
                                mode == 2,
                                false,
                                enabled,
                                cx,
                            ))),
                    )
                    .child(
                        div()
                            .mb(surface::css(5.))
                            .font_family("Roboto")
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(rgb(0xcccccc))
                            .child(t("SOURCE")),
                    )
                    .child(Select::new(&self.pip_source).disabled(!enabled)),
            )
            .into_any_element()
    }

    /// `.PIPContainer_btn_custom__8FMUe`: a 144x80 button carrying the shared
    /// `.btn` metrics (12px/14px white label, 6px/7px padding, `margin:10px 10px 0
    /// 0`, `:active{opacity:.6}`) and a miniature display graphic: `child_pip`
    /// 51x28 at the bottom-right with its right and bottom borders removed,
    /// `child_pbp` filling the right half with the PBP label above it
    /// (`z-index:2`, 40% down). The selected mode paints `#222` with a `#44d62c`
    /// border.
    fn pip_mode_button(
        &self,
        active: bool,
        pip: bool,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let target = if pip { 1 } else { 2 };
        let id = if pip {
            "accessory-pip-mode-pip"
        } else {
            "accessory-pip-mode-pbp"
        };
        let mut button = BaseButton::new(id)
            .w(surface::css(144.))
            .h(surface::css(80.))
            .mt(surface::css(10.))
            .mr(surface::css(10.))
            .pt(surface::css(6.))
            .pb(surface::css(7.))
            .text_size(surface::css(12.))
            .line_height(surface::css(14.))
            .text_color(rgb(0xffffff))
            .active(|style| style.opacity(0.6))
            .relative()
            .border_1()
            .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
            .bg(if active {
                rgb(0x222222)
            } else {
                rgba(0x00000000)
            })
            .disabled(!enabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    this.change("/secondDisplay/mode", json!(target), window, cx);
                }
            }));
        if pip {
            button = button.child(div().child(t("PIP_HEADER"))).child(
                div()
                    .absolute()
                    .right(px(0.))
                    .bottom(px(0.))
                    .w(surface::css(51.))
                    .h(surface::css(28.))
                    .bg(rgb(0x222222))
                    .border_1()
                    .border_r_0()
                    .border_b_0()
                    .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) }),
            );
        } else {
            button = button
                .child(
                    div()
                        .absolute()
                        .right(px(0.))
                        .top(px(0.))
                        .h_full()
                        .w(surface::css(72.))
                        .bg(rgb(0x222222))
                        .border_l_1()
                        .border_color(rgb(0x5d5d5d)),
                )
                .child(
                    div()
                        .absolute()
                        .top(surface::css(32.))
                        .left(px(0.))
                        .right(px(0.))
                        .child(div().text_center().child(t("PBP"))),
                );
        }
        button.into_any_element()
    }

    /// `RefreshRateCounter_main/row/btn_custom`: the 2x2 corner grid rendered on
    /// the `qAA` screen mock. `.row` is `height:27px` with `space-between`;
    /// buttons are held at `min-width:90px`, transparent with a `1px dashed #ccc`
    /// border and a `#` mark. The selected corner is `btn-green` (`#44d62c` on
    /// black text), hover paints `#fffafa1a`, and press paints `#0000004d` with a
    /// `#44d62c` border. `.RefreshRateCounter_middle` shows the `DISPLAY` label
    /// between the rows, and the body carries `.featureDisabled` while the counter
    /// is off. Values come from the monitor's `e3` enum (1..4).
    fn corner_grid(&self, path: &str, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let enabled = enabled && self.allowed(path);
        let selected = self.number(path);
        let row = |ids: [i64; 2]| {
            h_flex()
                .w_full()
                .h(surface::css(27.))
                .justify_between()
                .children(ids.into_iter().map(|id| {
                    let active = selected == id;
                    let target = path.to_owned();
                    BaseButton::new(SharedString::from(format!(
                        "accessory-corner-{target}-{id}"
                    )))
                    .min_w(surface::css(90.))
                    .h_full()
                    .p_0()
                    .border_1()
                    .border_dashed()
                    .border_color(if active { rgb(0x44d62c) } else { rgb(0xcccccc) })
                    .bg(if active {
                        rgb(0x44d62c)
                    } else {
                        rgba(0x00000000)
                    })
                    .text_color(if active { rgb(0x000000) } else { rgb(0xcccccc) })
                    .when(!active, |button| {
                        button
                            .hover(|style| style.bg(rgba(0xfffafa1a)))
                            .active(|style| style.bg(rgba(0x0000004d)).border_color(rgb(0x44d62c)))
                    })
                    .disabled(!enabled)
                    .child(if active { "#" } else { "" })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if enabled {
                            this.change(&target, json!(id), window, cx);
                        }
                    }))
                }))
        };
        self.screen_mock(
            v_flex()
                .size_full()
                .p(surface::css(10.))
                .when(!enabled, |body| body.opacity(0.3))
                .child(row([1, 2]))
                .child(
                    div()
                        .flex()
                        .flex_grow(2.)
                        .items_center()
                        .justify_center()
                        .text_center()
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .text_color(rgb(0x999999))
                        .child(t("DISPLAY")),
                )
                .child(row([3, 4]))
                .into_any_element(),
        )
    }

    /// `STA` feature row: `.slider_header.mb10` (feature name, the inline
    /// `.help`/`.tip` control at `margin-left:5px`, and — for the 0–100 sliders
    /// whose `maxStep` is 100 — the 50x27 numeric `<input>`), the slider itself,
    /// and the `.foot` range tags. `.foot` marks are absolutely placed at their
    /// share of the track: `.min{left:0}`, `.mid{left:0;width:100%;text-align:center}`,
    /// `.mid1{left:0;width:66%}`, `.mid2{left:0;width:133%}`, `.max{right:0}`.
    fn slider_row(
        &self,
        path: &str,
        label_key: &str,
        tooltip_key: Option<&str>,
        tags: Option<(&str, &str, &str, Option<&str>)>,
        numeric_input: bool,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = enabled && self.allowed(path);
        let mut header = h_flex().mb(surface::css(10.)).items_center().child(
            div()
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .child(t(label_key)),
        );
        if let Some(tooltip_key) = tooltip_key {
            header = header.child(self.help_control(
                SharedString::from(format!("accessory-help-{label_key}")),
                t(tooltip_key),
                cx,
            ));
        }
        if numeric_input {
            let input = self.inputs.get(path);
            let mut field = div()
                .ml(surface::css(10.))
                .w(surface::css(50.))
                .h(surface::css(27.))
                .bg(rgb(0x111111))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .px(surface::css(6.))
                .py(surface::css(5.))
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(rgb(0xcccccc));
            if let Some(input) = input {
                field = field.child(
                    Input::new(input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .disabled(!enabled)
                        .h(surface::css(17.))
                        .w_full()
                        .p_0(),
                );
            }
            header = header.child(field);
        }
        let mut row = v_flex()
            .id(SharedString::from(format!("accessory-slider-row-{path}")))
            .w_full()
            .when(!enabled, |row| row.opacity(0.3))
            .child(header);
        if let Some(slider) = self.sliders.get(path) {
            row = row.child(
                div()
                    .h(surface::css(40.))
                    .flex()
                    .items_center()
                    // `.slider-container{height:64px}` keeps the track 25px above
                    // the tags; the retained slider owns its own track metrics.
                    .child(Slider::new(slider).disabled(!enabled)),
            );
        }
        if let Some((min, mid, max, boost)) = tags {
            row = row.child(surface::slider_tags(min, Some(mid), max, boost));
        }
        row.into_any_element()
    }

    /// `RTA`: the monitor Game Mode widget. `wrA` gives it
    /// `title: SCARLETT_GAME_MODE_HEADER`, `tips: SCARLETT_GAME_MODE_TOOLTIP`,
    /// `hasSwitch: false` and `customStyle:{marginBottom:"30px"}`; the body is a
    /// `.btn_group` of preset buttons plus four `STA` rows — brightness and
    /// contrast as 0–100 sliders with the numeric field, overdrive (OFF/WEAK/
    /// STRONG) and gamma (1.4/1.8/2.2, plus 2.4 on 3880) as discrete sliders with
    /// range tags. Non-Native gamut on 3880 disables contrast and gamma.
    fn monitor_gaming(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let gamut_locked = self.spec.product_id == 3880 && self.gaming_value("gamut") != &json!(0);
        let selected = self.number("/gaming/selectedPreset");
        // `.foot` tags are locale keys (OFF/WEAK/STRONG) in the source.
        let overdrive_tags = {
            let (min, mid, max) = gaming_overdrive_tags();
            (t(min), t(mid), t(max))
        };
        let mut panel = surface::panel_with_control(
            t("SCARLETT_GAME_MODE_HEADER"),
            self.help_control(
                "accessory-game-mode-help",
                t("SCARLETT_GAME_MODE_TOOLTIP"),
                cx,
            ),
            cx,
        )
        // `customStyle:{marginBottom:"30px"}`.
        .mb(surface::css(30.))
        .child(
            h_flex().flex_wrap().gap(surface::css(10.)).children(
                [
                    (0, "SCARLETT_DEFAULT"),
                    (1, "FPS"),
                    (3, "MMO"),
                    (2, "RACING"),
                    (4, "STREAMING"),
                    (5, "SCARLETT_CUSTOM"),
                ]
                .map(|(id, label)| {
                    source_button(
                        SharedString::from(format!("accessory-gaming-preset-{id}")),
                        t(label),
                        selected == id,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.change("/gaming/selectedPreset", json!(id), window, cx);
                    }))
                }),
            ),
        )
        .child(self.slider_row(
            "/gaming/customData/brightness",
            "SCREEN_BRIGHTNESS_HEADER",
            Some("SCREEN_BRIGHTNESS_TOOLTIP"),
            None,
            true,
            true,
            cx,
        ))
        .child(self.slider_row(
            "/gaming/customData/contrast",
            "CONTRAST_HEADER",
            Some("CONTRAST_TOOLTIP"),
            None,
            true,
            !gamut_locked,
            cx,
        ))
        .child(self.slider_row(
            "/gaming/customData/overdrive",
            "OVERDRIVE_HEADER",
            Some("OVERDRIVE_TOOLTIP"),
            Some((
                overdrive_tags.0.as_str(),
                overdrive_tags.1.as_str(),
                overdrive_tags.2.as_str(),
                None,
            )),
            false,
            true,
            cx,
        ));
        panel = panel.child(self.slider_row(
            "/gaming/customData/gamma",
            "GAMMA_HEADER",
            Some("GAMMA_TOOLTIP"),
            Some(gaming_gamma_tags(self.spec.product_id)),
            false,
            !gamut_locked,
            cx,
        ));
        if self.spec.product_id == 3880 {
            let gamut = self.gaming_value("gamut").as_i64().unwrap_or(0);
            panel = panel
                .child(
                    h_flex()
                        .mb(surface::css(10.))
                        .items_center()
                        .child(
                            div()
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(t("COLOR_GAMUT")),
                        )
                        .child(self.help_control(
                            "accessory-color-gamut-help",
                            t("COLOR_GAMUT_TOOLTIP"),
                            cx,
                        )),
                )
                // `krA`: NATIVE, REC 709 and DCI-P3 as `.btn_group` buttons.
                // `B4` is NATIVE 0, DCI_P3 1, REC_709 2.
                .child(h_flex().flex_wrap().gap(surface::css(10.)).children(
                    [(0, "NATIVE"), (2, "REC 709"), (1, "DCI-P3")].map(|(id, label)| {
                        source_button(
                            SharedString::from(format!("accessory-color-gamut-{id}")),
                            label.into(),
                            gamut == id,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.change("/gaming/customData/gamut", json!(id), window, cx);
                            },
                        ))
                    }),
                ));
            // `zrA` renders the warning only while the gamut is not Native.
            if gamut != 0 {
                panel = panel.child(
                    h_flex()
                        .items_center()
                        .text_color(cx.theme().foreground)
                        .child(
                            // `.exclamationText span:before`: a 14px `#5d5d5d`
                            // disc carrying the shared exclamation glyph.
                            div()
                                .w(surface::css(14.))
                                .h(surface::css(14.))
                                .mr(surface::css(5.))
                                .rounded_full()
                                .bg(rgb(0x5d5d5d))
                                .flex_shrink_0()
                                .child(
                                    img("synapse/accessory-exclamation.svg")
                                        .size(surface::css(14.)),
                                ),
                        )
                        .child(
                            div()
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(t("COLOR_GAMUT_WARNING")),
                        ),
                );
            }
        }
        panel.into_any_element()
    }

    /// `wAA`（3858）/`nSA`（3880）：Color Temperature 组件。`wrA` 带
    /// `title: COLOR_TEMPERATURE_HEADER`、`tips: COLOR_PROFILE_TOOLTIP`、
    /// `hasSwitch:false`；组件体先是 `.btn_group` 的六个 `GM` 预设按钮，然后是被
    /// `.slide-off`/`.slide-off.slide-on`（`display:none`/`display:block`，因此是
    /// 立即显隐而非动画）包着的红/绿/蓝三行 `STA`——它们没有 `tooltipText`、也没传
    /// `enableSliderRange`（`.foot` 灰标因此为空），但 `maxStep` 默认 100，所以带
    /// 数值输入框。
    fn color_temperature_widget(&self, cx: &Context<Self>) -> AnyElement {
        let selected = self.number("/color/selectedPreset");
        let mut panel =
            surface::panel_with_control(
                t("COLOR_TEMPERATURE_HEADER"),
                self.help_control(
                    "accessory-color-temperature-help",
                    t("COLOR_PROFILE_TOOLTIP"),
                    cx,
                ),
                cx,
            )
            .child(h_flex().flex_wrap().gap(surface::css(10.)).children(
                COLOR_PRESETS.map(|(id, label)| {
                    source_button(
                        SharedString::from(format!("accessory-color-preset-{id}")),
                        t(label),
                        selected == id,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.change("/color/selectedPreset", json!(id), window, cx);
                    }))
                }),
            ));
        if selected == 11 {
            for (color, label) in [("red", "RED"), ("green", "GREEN"), ("blue", "BLUE")] {
                panel = panel.child(self.slider_row(
                    &format!("/color/customData/{color}"),
                    label,
                    None,
                    None,
                    true,
                    true,
                    cx,
                ));
            }
        }
        panel.into_any_element()
    }

    /// `rSA`：THX Cinema 组件——`tTA` 外壳（`title: THX_CINEMA_HEADER`、
    /// `tips: THX_CINEMA_TOOLTIP`、`hasSwitch:!disabledReason`、
    /// `active: !disabledReason && isEnabled`、`customStyle:{zIndex:3}`），
    /// 组件体是 `THX_CINEMA_DESC` 文本（有禁用原因时带 `.featureDisabled`）。
    fn thx_cinema_widget(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.checked("/thxCinema/isEnabled");
        surface::panel_with_title_switch(
            t("THX_CINEMA_HEADER"),
            surface::SynapseSwitch::new("accessory-thx-cinema")
                .accessibility_label(t("THX_CINEMA_HEADER"))
                .checked(enabled)
                .on_change(cx.listener(|this, next: &bool, window, cx| {
                    this.change("/thxCinema/isEnabled", json!(*next), window, cx);
                })),
            self.help_control("accessory-thx-cinema-help", t("THX_CINEMA_TOOLTIP"), cx),
            cx,
        )
        .child(
            div()
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .child(t("THX_CINEMA_DESC")),
        )
        .into_any_element()
    }

    /// `ISA`：HDR 组件——`tTA` 外壳（`title: HDR_HEADER`、`tips:` Windows 11 用
    /// `HDR_TOOLTIP_WINDOWS_11`、否则 `HDR_TOOLTIP`、`hasSwitch:!disabledReason`、
    /// `active: !disabledReason && isHdrEnabled`），组件体是 `HDR_MSG`。
    fn hdr_widget(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.checked("/hdr/isEnabled");
        surface::panel_with_title_switch(
            t("HDR_HEADER"),
            surface::SynapseSwitch::new("accessory-hdr")
                .accessibility_label(t("HDR_HEADER"))
                .checked(enabled)
                .on_change(cx.listener(|this, next: &bool, window, cx| {
                    this.change("/hdr/isEnabled", json!(*next), window, cx);
                })),
            self.help_control(
                "accessory-hdr-help",
                t(hdr_tooltip_key(system::is_windows_11())),
                cx,
            ),
            cx,
        )
        .child(
            div()
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .child(t("HDR_MSG")),
        )
        .into_any_element()
    }

    /// `RSA`：Color Profile 组件——`tTA` 外壳（`title:
    /// PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER`、`tips:
    /// PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_TOOLTIP`），组件体是
    /// `.screen-refresh-container`（禁用时 `.featureDisabled`）里的配置文件下拉
    /// （`k6O`，数据来自扩展上报的 `colorProfiles`；原版在选项 ≤1 时禁用，且在没有
    /// 配置时用一条空选项兜底）与 `.img-text .external` 外链
    /// （`PERFORMANCE_EXTERNAL_DISPLAY_COLOR_MANAGER`，点击拉起 Windows 颜色管理）。
    fn color_profile_widget(&self, cx: &Context<Self>) -> AnyElement {
        let select = Select::new(&self.color_profile)
            .disabled(true)
            .w(surface::css(360.));
        surface::panel_with_control(
            t("PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER"),
            self.help_control(
                "accessory-color-profile-help",
                t("PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_TOOLTIP"),
                cx,
            ),
            cx,
        )
        .child(select)
        .child(
            // `.img-text .external{color:#ccc;font-size:14px;line-height:44px;
            //  text-decoration:underline;text-transform:capitalize}` 与
            // `:hover{color:#44d62c}`、`:active{opacity:.7}`。
            div()
                .id("accessory-color-management-link")
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(44.))
                .text_color(rgb(0xcccccc))
                .underline()
                .hover(|style| style.text_color(rgb(0x44d62c)))
                .active(|style| style.opacity(0.7))
                .on_click(|_, _, _| {
                    let _ = system::open_color_management();
                })
                .child(t("PERFORMANCE_EXTERNAL_DISPLAY_COLOR_MANAGER")),
        )
        .into_any_element()
    }

    /// `COLOR_PAGE_COLUMNS` 的标题键对应的组件。
    fn color_widget(&self, title_key: &str, cx: &Context<Self>) -> AnyElement {
        match title_key {
            "THX_CINEMA_HEADER" => self.thx_cinema_widget(cx),
            "PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER" => self.color_profile_widget(cx),
            "HDR_HEADER" => self.hdr_widget(cx),
            _ => self.color_temperature_widget(cx),
        }
    }

    fn monitor_color(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        if self.spec.product_id != 3880 {
            return v_flex()
                .gap_5()
                .child(self.color_temperature_widget(cx))
                .into_any_element();
        }
        // `.body-widgets{flex-direction:row;flex-wrap:wrap;justify-content:center;
        //  margin:auto;max-width:1240px}` 与 `.widget-col{width:600px}`。
        h_flex()
            .flex_wrap()
            .justify_center()
            .items_start()
            .children(COLOR_PAGE_COLUMNS.map(|column| {
                v_flex()
                    .w(surface::css(600.))
                    .children(column.map(|title_key| self.color_widget(title_key, cx)))
            }))
            .into_any_element()
    }

    /// `SSA` primary input source group: `.btn_group.inputSource` (`gap:20px`)
    /// holding `.btn_custom` buttons — transparent, `1px solid #5d5d5d`, Roboto
    /// 12px uppercase, `padding:7px 16px 6px`, `margin:0`, `width:fit-content`,
    /// and `.active{background-color:#222;border-color:#44d62c}`.
    ///
    /// Each button carries an `<img>` (`height:20px;width:40px`) from the bundle:
    /// `icon_hdmi.7df743db.svg`, `icon_displayport.c7f9208d.svg`,
    /// `icon_usb_typec.169c5217.svg` and the Auto icon from module 6370. Those
    /// files are missing from the fetched `.ref/devices/3858/static/` tree (it has
    /// only `css/` and `js/`), so the buttons render label-only: the artwork is
    /// recorded as unavailable rather than invented.
    fn input_source_group(&self, cx: &Context<Self>) -> AnyElement {
        let current = self.number("/inputSource");
        h_flex()
            .flex_wrap()
            .gap(surface::css(20.))
            .children(INPUT_SOURCES.map(|(id, key)| {
                let active = current == id;
                source_button(
                    SharedString::from(format!("accessory-input-source-{id}")),
                    t(key),
                    active,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.request_input_source(id, window, cx);
                }))
            }))
            .into_any_element()
    }

    /// `SSA.confirmInputSourceChange`: an unchanged source is ignored, otherwise
    /// the change applies at once while the prompt is disabled.
    fn request_input_source(&mut self, source: i64, window: &mut Window, cx: &mut Context<Self>) {
        match input_source_step(self.number("/inputSource"), source, self.ask_again) {
            InputSourceStep::Ignore => {}
            InputSourceStep::Prompt => {
                self.pending_input_source = Some(source);
                cx.notify();
            }
            InputSourceStep::Apply => self.change("/inputSource", json!(source), window, cx),
        }
    }

    /// `OSA.confirmAction`: `true` applies the pending source, `false` (a click
    /// outside the alert) cancels without changing anything.
    fn confirm_input_source(
        &mut self,
        confirmed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pending = self.pending_input_source.take();
        if confirmed && let Some(source) = pending {
            self.change("/inputSource", json!(source), window, cx);
            return;
        }
        cx.notify();
    }

    /// `OSA` confirmation alert, mounted in the widget like
    /// `.alert_wrap > #confirmationAlert.alert.profile-del`: absolutely placed at
    /// `left:40px;top:165px` against the widget box, `min-width:300px`, `#111`
    /// surface, `1px solid #fd4949`, 3px radius, `box-shadow:0 6px 10px 0 #0003`,
    /// 20px padding, a centered column, and
    /// `transition:visibility 0s,opacity .3s linear` driven by `.show`.
    fn input_source_alert(
        &self,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::theme::ProfileAlertColors;
        use gpui_kit::base::motion::{self, Easing, Transition};
        let opacity = motion::transition(
            ("accessory-input-source-alert", "alert-opacity"),
            if active { 1.0 } else { 0.0 },
            Transition::new(std::time::Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let danger = ProfileAlertColors::new().danger();
        let rem = window.rem_size();
        // `SSA` puts the widget in `.widgetZIndex{z-index:6}` so the floating
        // alert covers the neighbouring widgets; `deferred` paints it in a later
        // pass with the same priority.
        deferred(
            div()
                .id("accessory-input-source-alert")
                .test_support()
                .absolute()
                .left(surface::css(40.))
                .top(surface::css(165.))
                .min_w(surface::css(300.))
                .flex()
                .flex_col()
                .items_center()
                .p(surface::css(20.))
                .rounded(surface::css(3.))
                .border_1()
                .border_color(danger)
                .bg(rgb(0x111111))
                .shadow(vec![BoxShadow {
                    color: rgba(0x00000033).into(),
                    offset: point(Pixels::ZERO, rem * (6. / 16.)),
                    blur_radius: rem * (10. / 16.),
                    spread_radius: Pixels::ZERO,
                    inset: false,
                }])
                .opacity(opacity)
                .when(!active, |alert| alert.invisible())
                // The source listens for a document mousedown and cancels; clicks
                // landing inside the alert must not reach that handler.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .mb(surface::css(10.))
                        .text_center()
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(rgb(0xcccccc))
                        .child(t_or("SCARLETT_CONFIRMATION_DESC", &t("CONFIRMATION_DESC"))),
                )
                .child(
                    div().mb(surface::css(10.)).self_start().child(
                        Checkbox::new("accessory-input-source-ask-again")
                            .label(t_or("SCARLETT_CONFIRMAION_TEXT", &t("CONFIRMAION_TEXT")))
                            .checked(!self.ask_again)
                            .disabled(!active)
                            .on_click(cx.listener(|this, next: &bool, _, cx| {
                                this.ask_again = !*next;
                                cx.notify();
                            })),
                    ),
                )
                .child(
                    BaseButton::new("accessory-input-source-confirm")
                        .h(surface::css(27.))
                        .min_w(surface::css(90.))
                        .px(surface::css(5.))
                        .py(surface::css(4.))
                        .font_family("Roboto")
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .whitespace_nowrap()
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .bg(danger)
                        .text_color(rgb(0x111111))
                        .disabled(!active)
                        .hover(|style| style.opacity(0.8))
                        .active(|style| style.opacity(0.6))
                        .child(t_or("SCARLETT_CONFIRM", &t("CONFIRM")))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_input_source(true, window, cx);
                        })),
                ),
        )
        .with_priority(6)
        .into_any_element()
    }

    fn monitor_display(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.checked("/secondDisplay/isEnabled");
        let asking = self.pending_input_source.is_some();
        let pip = surface::panel_with_control(
            t("PIP_HEADER"),
            self.help_control("accessory-pip-help", t("PIP_TOOLTIP"), cx),
            cx,
        )
        .child(self.toggle("/secondDisplay/isEnabled", t("PIP_HEADER"), true, cx))
        .child(self.pip_display(enabled, cx));
        let mut view = v_flex()
            .gap_5()
            // `OSA` listens on the document for a mousedown outside the alert and
            // cancels; a click inside the alert stops propagation before this.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.pending_input_source.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(
                surface::panel_with_control(
                    t("SCARLETT_INPUT_SOURCE_HEADER"),
                    self.help_control(
                        "accessory-input-source-help",
                        t("SCARLETT_INPUT_SOURCE_TOOLTIP"),
                        cx,
                    ),
                    cx,
                )
                .relative()
                .child(self.input_source_group(cx))
                .child(self.input_source_alert(asking, window, cx)),
            )
            .child(pip)
            .child(
                surface::panel_with_control(
                    t("FREE_SYNC_HEADER"),
                    self.help_control("accessory-free-sync-help", t("FREE_SYNC_TOOLTIP"), cx),
                    cx,
                )
                .child(self.toggle(
                    "/adaptiveSync/isEnabled",
                    t("FREE_SYNC_HEADER"),
                    true,
                    cx,
                )),
            )
            .child(
                surface::panel_with_control(
                    t("FPS_COUNTER_HEADER"),
                    self.help_control("accessory-fps-counter-help", t("FPS_COUNTER_TOOLTIP"), cx),
                    cx,
                )
                .child(self.toggle(
                    "/refeshRateCounter/isEnabled",
                    t("FPS_COUNTER_HEADER"),
                    true,
                    cx,
                ))
                .child(self.corner_grid(
                    "/refeshRateCounter/position",
                    self.checked("/refeshRateCounter/isEnabled"),
                    cx,
                )),
            );
        if self.spec.product_id == 3858 {
            let hdr_tip = self.hdr_tip();
            view = view.child(
                surface::panel_with_control(
                    "HDR",
                    self.help_control("accessory-hdr-help", hdr_tip, cx),
                    cx,
                )
                .child(self.toggle("/hdr/isEnabled", "HDR".into(), true, cx)),
            );
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Implemented pages dispatch before the descriptor check: a page can have
        // a real local renderer without a generated descriptor table (the Raptor
        // monitor pages and the cooling pages are audited that way), and the
        // descriptor check only decides which pages stay unsupported.
        let content = match (self.spec.product_id, self.page.as_str()) {
            (3858 | 3880, "TAB_GAMING") => self.monitor_gaming(window, cx),
            (3858 | 3880, "TAB_COLOR") => self.monitor_color(window, cx),
            (3858 | 3880, "TAB_DISPLAY") => self.monitor_display(window, cx),
            (3900, "TAB_PERFORMANCE") => self.pwm(cx),
            (3893, "TAB_PERFORMANCE") => self.hanbo(cx),
            (3907, "TAB_PERFORMANCE") => self.cooling(cx),
            (3921, "TAB_CUSTOMIZE") => self.corex_fan(cx),
            _ if !supports_page(self.spec.product_id, &self.page) => {
                surface::note("此页面没有本地描述符；原版控件尚未提取。", cx).into_any_element()
            }
            _ => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        };
        super::product_surface::body().child(content)
    }
}

#[cfg(test)]
#[path = "accessory_system_products_tests.rs"]
mod tests;

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
