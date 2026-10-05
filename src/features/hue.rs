//! Current product 769. Bridge observations are session state, never a profile.
//! A development preview supplies explicit fixtures; the live page has no Hue
//! transport and cannot claim that discovery, pairing or removal succeeded.
use super::Choice;
use crate::{
    i18n,
    ui::{surface, theme::HueColors as Colors},
};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    select::{SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

mod bridge;
mod brightness;
mod effects;
mod onboarding;
mod preview;
#[cfg(test)]
mod tests;
pub(crate) use preview::open_preview;

#[derive(Deserialize)]
struct Spec {
    fallback: String,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    initial: Initial,
    archetypes: Vec<Archetype>,
    lighting: LightingSpec,
}
#[derive(Deserialize)]
struct LightingSpec {
    effects: Vec<EffectChoice>,
    defaults: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
struct EffectChoice {
    name: String,
    id: u32,
}
#[derive(Deserialize)]
struct Initial {
    hue: BridgeState,
    brightness: Brightness,
}
#[derive(Deserialize)]
struct Archetype {
    name: String,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("hue_data.json")).expect("audited Hue literals")
    })
}
fn text(key: &str) -> String {
    let spec = spec();
    spec.translations
        .get(&i18n::locale())
        .and_then(|locale| locale.get(key))
        .or_else(|| {
            spec.translations
                .get(&spec.fallback)
                .and_then(|locale| locale.get(key))
        })
        .cloned()
        .unwrap_or_else(|| i18n::t(key))
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Integration {
    #[default]
    Init,
    Scanning,
    ScanningIp,
    ScanFailed,
    ScanCancel,
    ScanSuccessful,
    WaitUserClickPair,
    Pairing,
    PairCancel,
    PairFailed,
    RetryPair,
}
impl Integration {
    fn manual(self) -> bool {
        matches!(self, Self::ScanFailed | Self::ScanCancel | Self::PairCancel)
    }
    fn loading(self) -> bool {
        matches!(self, Self::Scanning | Self::ScanningIp | Self::RetryPair)
    }
    fn pair_logo(self) -> bool {
        matches!(self, Self::Pairing | Self::PairFailed)
    }
    fn source_name(self) -> &'static str {
        match self {
            Self::Init => "INIT",
            Self::Scanning => "SCANNING",
            Self::ScanningIp => "SCANNING_IP",
            Self::ScanFailed => "SCAN_FAILED",
            Self::ScanCancel => "SCAN_CANCEL",
            Self::ScanSuccessful => "SCAN_SUCCESSFUL",
            Self::WaitUserClickPair => "WAIT_USER_CLICK_PAIR",
            Self::Pairing => "PAIRING",
            Self::PairCancel => "PAIR_CANCEL",
            Self::PairFailed => "PAIR_FAILED",
            Self::RetryPair => "RETRY_PAIR",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeState {
    is_paired: bool,
    bridge_enabled: bool,
    is_loading: bool,
    is_control: bool,
    groups: Vec<Group>,
    active_group: String,
    devices: Vec<Light>,
}
impl BridgeState {
    fn deduplicate_lights(&mut self) {
        let mut seen = BTreeSet::new();
        self.devices
            .retain(|light| seen.insert(light.device_container_id.clone()));
    }
    fn controls_enabled(&self) -> bool {
        self.bridge_enabled && !self.is_loading
    }
    fn counts(&self) -> (usize, usize, usize) {
        let connected = self.devices.iter().filter(|light| light.is_on).count();
        (
            connected,
            self.devices
                .iter()
                .map(|light| &light.name)
                .collect::<BTreeSet<_>>()
                .len(),
            self.devices.len() - connected,
        )
    }
}
#[derive(Clone, Deserialize)]
struct Group {
    id: String,
    name: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Light {
    device_container_id: String,
    region_id: u32,
    is_on: bool,
    name: String,
    product_name: String,
    raw_data: LightIdentity,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LightIdentity {
    physical_name: String,
    physical_arche_type: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Brightness {
    is_enabled: bool,
    value: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    is_global_brightness: Option<bool>,
}
impl Brightness {
    fn global(&self) -> bool {
        self.is_global_brightness.unwrap_or(true)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum BridgeAlert {
    Enable,
    Remove,
}

pub(crate) struct HueChanged;
/// Navigation request emitted by the advanced-effects Chroma entry.
/// No installation result is carried or inferred.
pub(crate) struct HueChromaRequested;
pub(crate) struct HueWorkspace {
    bridge: BridgeState,
    integration: Integration,
    brightness: Brightness,
    draft: Value,
    ip: [Entity<InputState>; 4],
    groups: Entity<SelectState<Vec<Choice>>>,
    global_brightness: Entity<SliderState>,
    light_brightness: BTreeMap<u32, Entity<SliderState>>,
    alert: Option<BridgeAlert>,
    preview: bool,
    last_command: Option<String>,
    subscriptions: Vec<Subscription>,
    light_subscriptions: Vec<Subscription>,
    effect: Entity<SelectState<Vec<Choice>>>,
    selected_effect: u32,
    effect_settings: BTreeMap<u32, Value>,
    colors: [Entity<ColorPickerState>; 2],
    color_boost: Entity<InputState>,
    // isChromaEnabled / installation status are global service state, outside
    // the device quickEffects profile. Preview values stay in this entity.
    advanced: bool,
    /// The current source mounts a one-shot advanced-effects tutorial dot when
    /// the quick-effects widget becomes usable. It is local UI state; the
    /// service does not provide (and this app must not invent) a persistence
    /// response for it.
    tutorial_visible: bool,
    chroma_installed: Option<bool>,
    chroma_profiles: Entity<SelectState<Vec<Choice>>>,
}
impl EventEmitter<HueChanged> for HueWorkspace {}
impl EventEmitter<HueChromaRequested> for HueWorkspace {}
impl HueWorkspace {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ip = std::array::from_fn(|_| {
            cx.new(|cx| {
                InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 3)
            })
        });
        let groups = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let global_brightness = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(0.)
        });
        let effect = cx.new(|cx| {
            SelectState::new(
                spec()
                    .lighting
                    .effects
                    .iter()
                    .map(|e| Choice::new(e.id.to_string(), i18n::t(&e.name)))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        let colors = std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx)));
        let color_boost = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| super::lighting_input::valid_color_boost_draft(value))
        });
        let chroma_profiles = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let mut this = Self {
            bridge: spec().initial.hue.clone(),
            integration: Integration::Init,
            brightness: spec().initial.brightness.clone(),
            draft: json!({}),
            ip,
            groups,
            global_brightness,
            light_brightness: BTreeMap::new(),
            alert: None,
            preview: false,
            last_command: None,
            subscriptions: vec![],
            light_subscriptions: vec![],
            effect,
            selected_effect: 3,
            effect_settings: BTreeMap::new(),
            colors,
            color_boost,
            advanced: false,
            tutorial_visible: false,
            chroma_installed: None,
            chroma_profiles,
        };
        for octet in 0..4 {
            this.subscriptions.push(cx.subscribe_in(
                &this.ip[octet],
                window,
                move |_, input, event, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let value = input.read(cx).value().to_string();
                    let normalized = onboarding::normalize_octet(&value);
                    if normalized != value {
                        input.update(cx, |input, cx| input.set_value(normalized, window, cx));
                    }
                },
            ));
        }
        this.subscriptions.push(
            cx.subscribe_in(&this.groups, window, |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if this.preview
                        && this.bridge.bridge_enabled
                        && this.bridge.groups.iter().any(|g| g.id == *id)
                    {
                        this.bridge.active_group = id.clone();
                        this.last_command = Some(format!("ON_SET_ACTIVE_GROUP: {id}"));
                        cx.notify();
                    }
                }
            }),
        );
        this.subscriptions.push(cx.subscribe_in(
            &this.global_brightness,
            window,
            |this, _, event, _, cx| {
                if let SliderEvent::Change(value) = event {
                    if this.editable_brightness() && this.brightness.global() {
                        this.brightness.value = value.start().round().clamp(0., 100.) as u8;
                        this.changed(cx);
                    }
                }
            },
        ));
        this.subscribe_effects(window, cx);
        this.sync_effects(window, cx);
        this
    }
    pub(crate) fn snapshot(&self) -> Value {
        let mut draft = self.draft.clone();
        draft["brightness"] = serde_json::to_value(&self.brightness).expect("brightness value");
        draft["quickEffects"] = json!({"selectedEffectId":self.selected_effect,"selectedEffectSetting":self.effect_setting()});
        draft
    }
    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = json!({});
        // Explicit profile schema: observations, command status and Chroma's
        // global toggle cannot be restored even from an old/malformed draft.
        for key in ["brightness", "quickEffects", "ports"] {
            if let Some(value) = saved.and_then(|saved| saved.get(key)) {
                self.draft[key] = value.clone();
            }
        }
        self.brightness = self
            .draft
            .get("brightness")
            .cloned()
            .and_then(|v| serde_json::from_value::<Brightness>(v).ok())
            .filter(|v| v.value <= 100)
            .unwrap_or_else(|| spec().initial.brightness.clone());
        self.alert = None;
        self.restore_effects(window, cx);
        self.sync_brightness(window, cx);
        cx.notify();
    }
    pub(crate) fn dismiss_transient(&mut self, cx: &mut Context<Self>) {
        self.alert = None;
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        if !self.preview {
            cx.emit(HueChanged);
        }
        cx.notify();
    }
    fn editable_brightness(&self) -> bool {
        self.bridge.controls_enabled() && self.brightness.is_enabled
    }
    fn sync_brightness(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.global_brightness.update(cx, |slider, cx| {
            slider.set_value(self.brightness.value as f32, window, cx)
        });
        for (region, slider) in &self.light_brightness {
            let value = self.port_brightness(*region);
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
    }
    fn port_brightness(&self, region: u32) -> f32 {
        self.draft
            .get("ports")
            .and_then(Value::as_array)
            .and_then(|ports| {
                ports
                    .iter()
                    .find(|p| p.get("id").and_then(Value::as_u64) == Some(region as u64))
            })
            .and_then(|p| p.pointer("/brightness/value"))
            .and_then(Value::as_f64)
            .filter(|v| (0.0..=100.0).contains(v))
            .unwrap_or(50.) as f32
    }
    fn set_integration(&mut self, next: Integration, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview {
            return;
        }
        if next.manual() && !self.integration.manual() {
            for input in &self.ip {
                input.update(cx, |input, cx| input.set_value("", window, cx));
            }
        }
        let ip = if next == Integration::ScanningIp {
            format!(
                "; IP={}",
                self.ip
                    .iter()
                    .map(|i| i.read(cx).value().to_string())
                    .collect::<Vec<_>>()
                    .join(".")
            )
        } else {
            String::new()
        };
        self.integration = next;
        self.last_command = Some(format!(
            "ON_SET_INTEGRATION_STATUS: {}{ip}",
            next.source_name()
        ));
        cx.notify();
    }
}
impl Render for HueWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.bridge.is_paired {
            return self.onboarding(cx);
        }
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .child(self.bridge_card(cx))
                    .child(self.effects_card(cx)),
            ))
            .child(surface::page_column(
                v_flex()
                    .child(self.brightness_card(cx))
                    .child(self.devices_card(cx)),
            ))
            .into_any_element()
    }
}
fn widget(title: &str, tip: Option<&str>, control: impl IntoElement, cx: &App) -> Div {
    let mut card = v_flex()
        .relative()
        .w(surface::css(600.))
        .my(surface::css(10.))
        .py(surface::css(30.))
        .px(surface::css(40.))
        .rounded(surface::css(5.))
        .bg(cx.theme().group_box)
        .text_color(cx.theme().foreground)
        .text_size(surface::css(14.))
        .line_height(surface::css(20.));
    if !title.is_empty() {
        card = card.child(
            h_flex()
                .items_start()
                .gap(surface::css(10.))
                .mb(surface::css(20.))
                .child(
                    div()
                        .font_family("RazerF5")
                        .text_size(surface::css(16.))
                        .text_color(cx.theme().primary)
                        .child(text(title).to_uppercase()),
                )
                .child(div().mt(surface::css(3.)).child(control)),
        );
    }
    if let Some(tip) = tip {
        let label = text(tip);
        card = card.child(
            gpui_kit::base::Button::new(SharedString::from(format!("hue-help-{title}")))
                .absolute()
                .top(surface::css(12.))
                .right(surface::css(12.))
                .size(surface::css(16.))
                .p_0()
                .accessibility_label(label.clone())
                .child(img("synapse/onboard-help.svg").size_full())
                .tooltip(move |window, cx| {
                    tooltip::Tooltip::new(label.clone())
                        .max_w(surface::css(350.))
                        .build(window, cx)
                }),
        );
    }
    card
}
fn command(
    id: &'static str,
    key: &str,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .accessibility_label(text(key))
        .disabled(disabled)
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(16.))
        .py_0()
        .flex()
        .items_center()
        .justify_center()
        .text_size(surface::css(12.))
        .border_1()
        .border_color(Colors::button_border())
        .rounded(surface::css(3.))
        .bg(if primary {
            cx.theme().primary
        } else {
            Colors::secondary()
        })
        .text_color(if primary {
            Colors::primary_text()
        } else {
            Colors::secondary_text()
        })
        .focus_visible(|s| s.border_color(cx.theme().ring))
        .hover(|s| s.opacity(0.8))
        .when(disabled, |s| s.opacity(0.3))
        .child(text(key).to_uppercase())
}
fn logo(pair: bool, loading: bool) -> Div {
    div()
        .relative()
        .w(surface::css(if pair { 93. } else { 80. }))
        .h(surface::css(if pair { 89. } else { 80. }))
        .child(
            img(if pair {
                "synapse/hue-hue_pair_logo.svg"
            } else {
                "synapse/hue-hue_scan_logo.svg"
            })
            .size_full(),
        )
        .when(loading, |v| {
            v.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(img("synapse/hue-preloader.svg").size(surface::css(44.))),
            )
        })
}
fn light_icon(archetype: &str) -> AnyElement {
    let name = if spec().archetypes.iter().any(|a| a.name == archetype) {
        archetype
    } else {
        "default"
    };
    img(SharedString::from(format!("synapse/hue-light-{name}.svg")))
        .size(surface::css(20.))
        .object_fit(ObjectFit::Contain)
        .into_any_element()
}
