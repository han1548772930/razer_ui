//! Current mouse QuickEffects controls; source evidence is generated from current official bundles.
use super::*;
use crate::features::controls::Choice;
use crate::features::lighting_color::LightingColorPicker;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::select::{SelectEvent, SelectState};
use razer_widgets::source_slider::SourceSlider;
use razer_widgets::stepper::{Stepper, StepperEvent};
use razer_widgets::theme::{KeyboardEffectColors, KeyboardRgbEffectColors as Colors};

#[derive(Deserialize)]
struct SourceLighting {
    product_id: u32,
    shared_quick_renderer: bool,
    #[serde(default)]
    page_variant: String,
    config: Value,
}
fn source_lighting(product_id: u32) -> Option<&'static SourceLighting> {
    static SOURCE: OnceLock<Vec<SourceLighting>> = OnceLock::new();
    SOURCE
        .get_or_init(|| read_mouse_lighting_current_catalog())
        .iter()
        .find(|source| source.product_id == product_id)
}
pub(super) fn lighting_supported(product_id: u32) -> bool {
    source_lighting(product_id).is_some_and(|source| source.shared_quick_renderer)
}
pub(super) fn mapping_target_info(product_id: u32) -> Option<&'static Value> {
    source_lighting(product_id).map(|source| &source.config["DeviceInfo"])
}

#[derive(Clone)]
pub struct MouseLightingRequested {
    owner: EntityId,
    generation: u64,
    request_id: u64,
    profile_guid: Option<String>,
    product_id: u32,
    command: &'static str,
    payload: Value,
}
impl MouseLightingRequested {
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn profile_guid(&self) -> Option<&str> {
        self.profile_guid.as_deref()
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn command(&self) -> &str {
        self.command
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}
impl EventEmitter<MouseLightingRequested> for MouseProductWorkspace {}

#[derive(IntoElement)]
struct MouseChromaInstallSpinner;
impl RenderOnce for MouseChromaInstallSpinner {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        svg()
            .path("synapse/keyboard-chroma-install-spinner.svg")
            .size(surface::css(16.))
            .text_color(Colors::background())
            .with_animation(
                "mouse-chroma-install-spin",
                Animation::new(std::time::Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                |icon, phase| icon.with_transformation(Transformation::rotate(percentage(phase))),
            )
    }
}

pub(super) struct State {
    observation: Value,
    cached_effect_settings: BTreeMap<String, Vec<Value>>,
    cached_restore_requested: bool,
    ui_advanced: Option<bool>,
    ui_chroma_profile: Option<String>,
    custom_colors_draft: Option<Vec<Value>>,
    sync_started: Option<std::time::Instant>,
    sync_animation: u64,
    generation: u64,
    request_id: u64,
    pending: BTreeMap<u64, &'static str>,
    latest: BTreeMap<&'static str, u64>,
    error: Option<String>,
    pub(super) effect: Entity<SelectState<Vec<Choice>>>,
    chroma_profile: Entity<SelectState<Vec<Choice>>>,
    colors: [Entity<ColorPickerState>; 2],
    power_indicator_color: Entity<ColorPickerState>,
    duration: Entity<SliderState>,
    boost: Entity<Stepper>,
}

impl MouseProductWorkspace {
    /// Shared OBM/header reducer observation, independent of the selected tab.
    pub fn observe_mouse_adjustment_mode(
        &mut self,
        running: bool,
        show_prompt: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        self.adjustment_mode_running = running;
        if let Some(show) = show_prompt {
            self.adjustment_mode_prompt = show;
        }
        if !running {
            self.adjustment_mode_prompt = false;
        }
        cx.notify();
    }
    /// Called only by controls whose current source checks adjustment mode.
    pub(super) fn check_mouse_adjustment_mode(&mut self, cx: &mut Context<Self>) -> bool {
        // The /162 Babel controls have no adjustment-mode interception.
        if self.page == "TAB_LIGHTING" && self.mouse_lighting_page_variant() == "babel_162" {
            return false;
        }
        if self.adjustment_mode_running {
            self.adjustment_mode_prompt = true;
            cx.notify();
            true
        } else {
            false
        }
    }
    fn mouse_lighting_page_variant(&self) -> &str {
        source_lighting(self.spec.product_id).map_or("", |source| source.page_variant.as_str())
    }
    pub(super) fn mouse_lighting_controls_disabled(&self) -> bool {
        matches!(
            self.mouse_lighting_page_variant(),
            "wdl" | "wdl_smart_dimming"
        ) && self.lighting_observation()["isDynamicLighting"] == true
            && self.lighting_observation()["isBle"] != true
    }
    pub(super) fn mouse_brightness_disabled(&self) -> bool {
        self.mouse_lighting_controls_disabled()
            || self.lighting_observation()["nanoLeafEnabled"] == false
    }
    pub(super) fn mouse_lighting_page_gates(
        &self,
        columns: AnyElement,
        iframe: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let unit = window.rem_size() / 16.;
        div()
            .relative()
            .min_h_full()
            .child(columns)
            .when(self.mouse_lighting_controls_disabled(), |page| {
                page.child(
                    div()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .bg(Colors::backdrop())
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .w(surface::css(400.))
                                .p(surface::css(20.))
                                .items_center()
                                .rounded(surface::css(3.))
                                .bg(Colors::background())
                                .border_1()
                                .border_color(Colors::warning())
                                .shadow(vec![BoxShadow {
                                    color: Colors::black(),
                                    offset: point(px(0.), unit * 6.),
                                    blur_radius: unit * 10.,
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .text_color(Colors::text())
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .text_center()
                                .child(
                                    img("synapse/keyboard-lighting-warning.svg")
                                        .size(surface::css(20.)),
                                )
                                .child(
                                    gpui_kit::base::TextView::html(
                                        "mouse-wdl-description",
                                        t("POPUP_WARNING_DYNAMIC_LIGHTING_AUDIO_VISUALIZER"),
                                    )
                                    .selectable(false)
                                    .scrollable(false),
                                )
                                .child(
                                    BaseButton::new("mouse-wdl-settings")
                                        .underline()
                                        .hover(|button| button.text_color(Colors::accent()))
                                        .child(t(if iframe {
                                            "MANAGE_CHROMA_APP_SETTINGS"
                                        } else {
                                            "MANAGE_SYNAPSE_SETTINGS"
                                        }))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.mouse_lighting_controls_disabled() {
                                                this.emit_lighting(
                                                    "openLightingSettings",
                                                    json!({"chroma":iframe}),
                                                    cx,
                                                );
                                            }
                                        })),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
    pub(super) fn mouse_smart_dimming(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !matches!(
            self.mouse_lighting_page_variant(),
            "wdl_smart_dimming" | "smart_dimming"
        ) {
            return None;
        }
        let brightness = self.boolean("/brightness/isEnabled");
        let enabled = self.draft["smartDimming"]["isEnabled"] == true;
        let value = self.draft["smartDimming"]["value"].as_u64().unwrap_or(25);
        let disabled = !brightness || self.mouse_lighting_controls_disabled();
        Some(
            surface::panel_with_title_switch(
                t("SMART_DIMMING_HEADER"),
                surface::SynapseSwitch::new("mouse-smart-dimming-enabled")
                    .accessibility_label(t("SMART_DIMMING_HEADER"))
                    .checked(brightness && enabled)
                    .disabled(disabled)
                    .on_change(cx.listener(move |this, next: &bool, _, cx| {
                        this.set_mouse_smart_dimming(value, *next, cx)
                    })),
                surface::help_control("mouse-smart-dimming-help", t("SMART_DIMMING_TOOLTIP")),
                cx,
            )
            .child(
                v_flex()
                    .opacity(if !brightness || !enabled { 0.3 } else { 1. })
                    .child(
                        div()
                            .mb(surface::css(20.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .child(t("SMART_DIMMING_DESC")),
                    )
                    .child(
                        h_flex()
                            .w(surface::css(166.))
                            .h(surface::css(36.))
                            .mb(surface::css(15.))
                            .px(surface::css(10.))
                            .items_center()
                            .rounded(surface::css(18.))
                            .border_1()
                            .border_color(Colors::border())
                            .bg(Colors::background())
                            .hover(|row| row.border_color(Colors::accent()))
                            .children([0u64, 25, 50].map(|choice| {
                                BaseButton::new(SharedString::from(format!(
                                    "mouse-smart-dimming-{choice}"
                                )))
                                .disabled(disabled || !enabled)
                                .w(surface::css(48.))
                                .h(surface::css(25.))
                                .rounded(surface::css(12.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(surface::css(14.))
                                .bg(if value == choice {
                                    Colors::accent()
                                } else {
                                    Colors::transparent()
                                })
                                .text_color(if value == choice {
                                    Colors::selected_text()
                                } else {
                                    Colors::text()
                                })
                                .child(format!("{choice}%"))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if value != choice {
                                            this.set_mouse_smart_dimming(choice, enabled, cx);
                                        }
                                    },
                                ))
                            })),
                    )
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child(t("SMART_DIMMING_INSTRUCTION")),
                    ),
            )
            .into_any_element(),
        )
    }
    fn set_mouse_smart_dimming(&mut self, value: u64, enabled: bool, cx: &mut Context<Self>) {
        if !self.boolean("/brightness/isEnabled") || self.mouse_lighting_controls_disabled() {
            return;
        }
        let setting = json!({"value":value,"isEnabled":enabled});
        set_pointer(&mut self.draft, "/smartDimming", setting.clone());
        self.emit_lighting("ON_SMART_DIMMING", json!({"smartDimming":setting}), cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    pub(super) fn mouse_power_indicator(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if self.mouse_lighting_page_variant() != "hyperflux"
            || self.lighting_observation()["isMatPlugged"] != true
        {
            return None;
        }
        let state = self.mouse_lighting.as_ref()?;
        let mouse_disabled = self.lighting_observation()["isMouse"] != true;
        Some(
            surface::panel_with_help(
                t("POWER_INDICATOR"),
                surface::help_control("mouse-power-indicator-help", t("TOOLTIP_POWER_INDICATOR")),
                cx,
            )
            .child(
                LightingColorPicker::new(&state.power_indicator_color, "")
                    .allow_none(false)
                    .disabled(mouse_disabled)
                    .on_custom_colors_commit(cx.listener(
                        |this, slots: &razer_state::CustomColorSlots, _, cx| {
                            this.commit_mouse_custom_colors(slots, cx)
                        },
                    )),
            )
            .child(div().h(surface::css(17.)))
            .children(
                [
                    ("isMouseIndicatorOn", "SHOW_MOUSE_INDICATOR", mouse_disabled),
                    ("isMatIndicatorOn", "SHOW_MOUSE_MAT_INDICATOR", false),
                ]
                .map(|(key, label, disabled)| {
                    let checked = self.draft["powerIndicator"][key] == true;
                    Checkbox::new(SharedString::from(format!("mouse-power-{key}")))
                        .label(t(label))
                        .checked(checked)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_mouse_power_indicator(key, json!(!checked), cx)
                        }))
                }),
            )
            .into_any_element(),
        )
    }
    fn set_mouse_power_indicator(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        if self.lighting_observation()["isMatPlugged"] != true
            || (key != "isMatIndicatorOn" && self.lighting_observation()["isMouse"] != true)
        {
            return;
        }
        set_pointer(&mut self.draft, &format!("/powerIndicator/{key}"), value);
        self.emit_lighting(
            "ON_SET_POWER_INDICATOR",
            json!({"powerIndicator":self.draft["powerIndicator"]}),
            cx,
        );
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    fn commit_mouse_custom_colors(
        &mut self,
        slots: &razer_state::CustomColorSlots,
        cx: &mut Context<Self>,
    ) {
        if !self.active || self.page != "TAB_LIGHTING" {
            return;
        }
        let colors: Vec<Value> = slots
            .iter()
            .map(|color| match color {
                Some([r, g, b]) => json!({"color":format!("#{r:02x}{g:02x}{b:02x}"),"picked":true}),
                None => json!({"color":"#111","picked":false}),
            })
            .collect();
        if let Some(state) = &mut self.mouse_lighting {
            state.custom_colors_draft = Some(colors.clone());
        }
        self.emit_lighting("SET_CUSTOM_COLORS", json!({"customColors":colors}), cx);
    }
    pub(super) fn mouse_lighting_config(&self) -> &'static Value {
        &source_lighting(self.spec.product_id)
            .expect("source lighting product")
            .config
    }
    pub(super) fn submit_mouse_lighting_field(
        &mut self,
        path: &str,
        previous_enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if !lighting_supported(self.spec.product_id) {
            return;
        }
        if path.starts_with("/brightness/") {
            let changed = previous_enabled != self.boolean("/brightness/isEnabled");
            self.emit_lighting(
                if changed {
                    "ON_CHANGE_ENABLE_STATE"
                } else {
                    "ON_SET_BRIGHTNESS"
                },
                json!({"brightness":self.draft["brightness"]}),
                cx,
            );
        } else if path.starts_with("/switchOffLighting/") {
            self.emit_lighting(
                "ON_SWITCH_OFF_SETTING",
                json!({"switchOffLighting":self.draft["switchOffLighting"]}),
                cx,
            );
        }
    }
    pub(super) fn lighting_observation(&self) -> &Value {
        static EMPTY: OnceLock<Value> = OnceLock::new();
        self.mouse_lighting
            .as_ref()
            .map(|state| &state.observation)
            .unwrap_or_else(|| EMPTY.get_or_init(|| json!({})))
    }

    pub(super) fn observe_mouse_runtime_lighting(&mut self, snapshot: &Value) {
        if !matches!(self.spec.product_id, 104 | 105) {
            return;
        }
        let Some(values) = self
            .mouse_lighting
            .as_mut()
            .and_then(|state| state.observation.as_object_mut())
        else {
            return;
        };
        if let Some(plugged) = snapshot["isMatPlugged"].as_bool() {
            values.insert("isMatPlugged".into(), json!(plugged));
        }
        // The current Hyperflux publisher sends runtimeData.isMouseConnected
        // and SET_IS_MOUSE together. Its Lighting selectors read isMouse from
        // specificDeviceReducer, rather than the Dashboard runtimeData name.
        if let Some(connected) = snapshot["isMouseConnected"].as_bool() {
            values.insert("isMouse".into(), json!(connected));
        }
    }

    pub(super) fn reset_mouse_runtime_lighting(&mut self) {
        if !matches!(self.spec.product_id, 104 | 105) {
            return;
        }
        if let Some(values) = self
            .mouse_lighting
            .as_mut()
            .and_then(|state| state.observation.as_object_mut())
        {
            for key in ["isMatPlugged", "isMouse"] {
                values.remove(key);
            }
        }
    }

    fn mouse_quick_enabled(&self) -> bool {
        let observed = self.lighting_observation();
        !self.mouse_lighting_controls_disabled()
            && observed["nanoLeafEnabled"] != false
            && observed["isChromaVisualizerEnabled"] != true
            && !(observed["usePowerSavingQuickEffect"] == true
                && observed["systemPowerStatus"] == "battery")
            && (self.rgb_hardware_effect() || !self.mouse_advanced())
    }
    fn mouse_advanced(&self) -> bool {
        self.mouse_lighting
            .as_ref()
            .and_then(|state| state.ui_advanced)
            .unwrap_or_else(|| self.lighting_observation()["isChromaEnabled"] == true)
    }
    pub(super) fn emit_lighting(
        &mut self,
        command: &'static str,
        payload: Value,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.mouse_lighting else {
            return;
        };
        state.error = None;
        state.request_id = state.request_id.wrapping_add(1);
        state
            .pending
            .retain(|_, pending_command| *pending_command != command);
        state.pending.insert(state.request_id, command);
        state.latest.insert(command, state.request_id);
        cx.emit(MouseLightingRequested {
            owner: cx.entity_id(),
            generation: state.generation,
            request_id: state.request_id,
            profile_guid: self.draft["guid"].as_str().map(str::to_owned),
            product_id: self.spec.product_id,
            command,
            payload,
        });
    }
    fn emit_mouse_effect(&mut self, cx: &mut Context<Self>) {
        let payload = json!({"effectSetting":{"selectedEffectId":self.rgb_effect_id(),
            "selectedEffectSetting":self.rgb_setting(),"regionId":0}});
        self.emit_lighting("ON_SET_EFFECTS", payload.clone(), cx);
        if self.mouse_lighting_config()["DeviceInfo"]["useHardwareEffectByMW"] == true {
            self.emit_lighting("ON_SET_HARDWARE_EFFECT", payload, cx);
        }
    }
    fn can_sync_mouse_effect(&self) -> bool {
        if !self.mouse_quick_enabled() {
            return false;
        }
        let observed = self.lighting_observation();
        if self.mouse_lighting_page_variant() == "babel_162" {
            // uJ.checkSyncQuickEffect counts the memory-event payload directly;
            // it does not exclude this product or consult validDevices.
            return observed["chromaDevices"]
                .as_array()
                .is_some_and(|devices| devices.len() > 1);
        }
        if self.rgb_effect_id() == 17 {
            return observed["validDevices"].as_array().is_some_and(|devices| {
                devices.iter().any(|device| {
                    device["productId"] != self.spec.product_id
                        && device["supportEffects"]
                            .as_array()
                            .is_some_and(|effects| effects.iter().any(|effect| effect["id"] == 17))
                })
            });
        }
        observed["chromaDevices"].as_array().is_some_and(|devices| {
            devices.len() > 1
                && devices
                    .iter()
                    .any(|device| device["productId"] != self.spec.product_id)
        })
    }
    fn sync_mouse_effect(&mut self, cx: &mut Context<Self>) {
        if !self.can_sync_mouse_effect() {
            return;
        }
        if let Some(state) = &mut self.mouse_lighting {
            if state
                .sync_started
                .is_none_or(|start| start.elapsed() >= std::time::Duration::from_millis(1200))
            {
                state.sync_started = Some(std::time::Instant::now());
                state.sync_animation = state.sync_animation.wrapping_add(1);
            }
        }
        let id = self.rgb_effect_id();
        let mut setting = self.rgb_setting();
        setting["effectId"] = json!(id);
        let custom: Vec<Value> = self
            .mouse_lighting
            .as_ref()
            .and_then(|state| state.custom_colors_draft.as_ref())
            .or_else(|| self.lighting_observation()["customColors"].as_array())
            .into_iter()
            .flatten()
            .filter(|color| {
                color["picked"] == true
                    && setting.as_object().is_some_and(|setting| {
                        setting.iter().any(|(key, value)| {
                            key.to_ascii_lowercase().starts_with("color")
                                && *value == color["color"]
                        })
                    })
            })
            .cloned()
            .collect();
        let mut payload =
            json!({"selectedEffectId":id,"effectSettings":setting,"customColors":custom});
        if id == 4 {
            let wave = &self.mouse_lighting_config()["DeviceInfo"]["WAVE_DIRECTION"];
            let direction = setting["direction"].as_u64();
            let index = match wave.as_str() {
                Some("CWCCW") => {
                    if direction == Some(12) {
                        0
                    } else {
                        1
                    }
                }
                Some("UpDown") => {
                    if direction == Some(4) {
                        0
                    } else {
                        1
                    }
                }
                Some("LeftRight") => {
                    if direction == Some(2) {
                        1
                    } else {
                        0
                    }
                }
                _ => {
                    if direction == Some(2) {
                        0
                    } else {
                        1
                    }
                }
            };
            payload["indexDirection"] = json!(index);
            payload["numberOfWaveDirection"] = json!(wave.as_array().map_or(2, Vec::len));
        }
        self.emit_lighting("ON_SYNC_QUICK_EFFECT", payload, cx);
        cx.notify();
    }
    fn mouse_sync_icon(&self) -> AnyElement {
        if let Some(state) = &self.mouse_lighting {
            if state
                .sync_started
                .is_some_and(|start| start.elapsed() < std::time::Duration::from_millis(1200))
            {
                return div()
                    .size(surface::css(26.))
                    .mr(surface::css(10.))
                    .with_animation(
                        ("mouse-chroma-sync", state.sync_animation),
                        Animation::new(std::time::Duration::from_millis(1200)).with_easing(linear),
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
                    .into_any_element();
            }
        }
        img("synapse/chroma_sync_v3_static.svg")
            .size(surface::css(26.))
            .mr(surface::css(10.))
            .into_any_element()
    }
    /// Observed reducer/device/module state only; local profile data is not an installation check.
    pub fn observe_mouse_lighting(
        &mut self,
        snapshot: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(running) = snapshot["adjustmentModeRunning"].as_bool() {
            self.observe_mouse_adjustment_mode(
                running,
                snapshot["showAdjustmentModePrompt"].as_bool(),
                cx,
            );
        } else if let Some(show) = snapshot["showAdjustmentModePrompt"].as_bool() {
            self.adjustment_mode_prompt = show;
        }
        let Some(state) = &mut self.mouse_lighting else {
            return;
        };
        if snapshot.get("isChromaEnabled").is_some() {
            state.ui_advanced = None;
        }
        if snapshot.get("selectedChromaProfileGuid").is_some() {
            state.ui_chroma_profile = None;
        }
        if snapshot.get("customColors").is_some() {
            state.custom_colors_draft = None;
        }
        if let (Some(target), Some(values)) =
            (state.observation.as_object_mut(), snapshot.as_object())
        {
            target.extend(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        }
        if let Some(cache) = snapshot
            .get("cachedEffectSettings")
            .and_then(Value::as_object)
        {
            // This is the source reducer's actual per-profile cache. A local
            // draft is never copied into it to fabricate a cache observation.
            state.cached_effect_settings = cache
                .iter()
                .filter_map(|(guid, settings)| {
                    settings
                        .as_array()
                        .map(|settings| (guid.clone(), settings.clone()))
                })
                .collect();
        }
        for key in ["selectedEffectId", "effectSettings"] {
            if let Some(value) = snapshot.get(key) {
                set_pointer(
                    &mut self.draft,
                    &format!("/quickEffects/{key}"),
                    value.clone(),
                );
            }
        }
        // Device/reducer notifications are authoritative. Synchronize the
        // retained controls under the same guard as the effect controls so a
        // read-back never becomes a second user submission.
        for key in ["brightness", "switchOffLighting"] {
            if let Some(value) = snapshot.get(key) {
                if let (Some(target), Some(fields)) =
                    (self.draft[key].as_object_mut(), value.as_object())
                {
                    target.extend(
                        fields
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone())),
                    );
                } else {
                    set_pointer(&mut self.draft, &format!("/{key}"), value.clone());
                }
            }
        }
        if let Some(value) = snapshot.get("smartDimming") {
            set_pointer(&mut self.draft, "/smartDimming", value.clone());
        }
        if let Some(value) = snapshot.get("powerIndicator") {
            set_pointer(&mut self.draft, "/powerIndicator", value.clone());
        }
        if let Some(colors) = snapshot.get("customColors").and_then(Value::as_array) {
            let slots = std::array::from_fn(|index| {
                let color = colors.get(index)?.get("color")?.as_str()?;
                if color == "#111" {
                    return None;
                }
                let hex = color.strip_prefix('#')?;
                if hex.len() != 6 {
                    return None;
                }
                let rgb = u32::from_str_radix(hex, 16).ok()?;
                Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
            });
            razer_state::CustomColors::ensure(cx);
            razer_state::CustomColors::replace(slots, cx);
        }
        if snapshot.get("cachedEffectSettings").is_some()
            && self
                .mouse_lighting
                .as_ref()
                .is_some_and(|state| state.cached_restore_requested)
        {
            self.restore_mouse_cached_effect_settings();
        }
        self.sync_mouse_lighting(window, cx);
        self.reconcile_mouse_mapping_lighting_70(window, cx);
        cx.notify();
    }
    pub fn accepts_mouse_lighting_request(
        &self,
        request: &MouseLightingRequested,
        cx: &Context<Self>,
    ) -> bool {
        request.owner == cx.entity_id()
            && request.product_id == self.spec.product_id
            && request.profile_guid.as_deref() == self.draft["guid"].as_str()
            && self.mouse_lighting.as_ref().is_some_and(|state| {
                state.generation == request.generation
                    && state.pending.contains_key(&request.request_id)
                    && state.latest.get(request.command) == Some(&request.request_id)
            })
    }
    pub fn reject_mouse_lighting_request(
        &mut self,
        request: &MouseLightingRequested,
        error: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_mouse_lighting_request(request, cx) {
            return false;
        }
        if let Some(state) = &mut self.mouse_lighting {
            state.pending.remove(&request.request_id);
            if request.command == "TOGGLE_CHROMA" {
                state.ui_advanced = None;
            }
            if request.command == "ON_SET_CHROMA_PROFILE" {
                state.ui_chroma_profile = None;
            }
            state.error = Some(error);
        }
        self.sync_mouse_lighting(window, cx);
        cx.notify();
        true
    }
    pub fn complete_mouse_lighting_request(
        &mut self,
        request: &MouseLightingRequested,
        snapshot: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_mouse_lighting_request(request, cx) {
            return false;
        }
        if let Some(state) = &mut self.mouse_lighting {
            state.pending.remove(&request.request_id);
        }
        if let Some(snapshot) = snapshot {
            self.observe_mouse_lighting(snapshot, window, cx);
        }
        cx.notify();
        true
    }
    pub(super) fn reset_mouse_lighting_session(&mut self, cx: &mut Context<Self>) {
        self.dismiss_mouse_lighting(cx);
        if let Some(state) = &mut self.mouse_lighting {
            state.generation = state.generation.wrapping_add(1);
            state.pending.clear();
            state.latest.clear();
            state.error = None;
            state.ui_advanced = None;
            state.ui_chroma_profile = None;
            state.custom_colors_draft = None;
            state.sync_started = None;
        }
    }
    pub fn mouse_lighting_error(&self) -> Option<&str> {
        self.mouse_lighting
            .as_ref()
            .and_then(|state| state.error.as_deref())
    }
    /// Invoke after the owner has subscribed. Babel's cache restoration is a
    /// local source reducer action, separate from host/module queries.
    pub fn request_mouse_lighting_observation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_lighting_page_variant() == "basic" {
            return;
        }
        if self.mouse_lighting_page_variant() == "babel_162" {
            self.emit_lighting(
                "getMemoryStorageItem",
                json!({"key":"lighting-engine-chroma-devices"}),
                cx,
            );
            if let Some(state) = &mut self.mouse_lighting {
                state.cached_restore_requested = true;
            }
            self.restore_mouse_cached_effect_settings();
            self.sync_mouse_lighting(window, cx);
        } else {
            self.emit_lighting("checkIsFullyInstall", json!({"module":"chroma-app"}), cx);
            self.emit_lighting(
                "getMemoryStorageItem",
                json!({"key":"lighting-engine-chroma-devices"}),
                cx,
            );
        }
        cx.notify();
    }
    fn restore_mouse_cached_effect_settings(&mut self) {
        if self.mouse_lighting_page_variant() != "babel_162" {
            return;
        }
        let Some(state) = &mut self.mouse_lighting else {
            return;
        };
        // GET_CACHED_EFFECT_SETTINGS uses selectedProfileGuid. Missing cache
        // leaves effectSettings unchanged, exactly as the current reducer.
        let settings = self.draft["guid"]
            .as_str()
            .and_then(|guid| state.cached_effect_settings.get(guid))
            .cloned();
        if state.observation.get("cachedEffectSettings").is_some() {
            state.cached_restore_requested = false;
        }
        if let Some(settings) = settings {
            set_pointer(
                &mut self.draft,
                "/quickEffects/effectSettings",
                json!(settings),
            );
        }
    }
    pub(super) fn mouse_effects(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.mouse_lighting_page_variant() == "basic" {
            return self.mouse_basic_effects(cx);
        }
        let observed = self.lighting_observation();
        let babel = self.mouse_lighting_page_variant() == "babel_162";
        let hardware = self.rgb_hardware_effect();
        let advanced = !hardware && self.mouse_advanced();
        let enabled = !self.mouse_lighting_controls_disabled()
            && observed["nanoLeafEnabled"] != false
            && observed["isChromaVisualizerEnabled"] != true;
        let tabs = h_flex()
            .h(surface::css(36.))
            .p(surface::css(5.))
            .border_1()
            .border_color(Colors::border())
            .rounded(surface::css(18.))
            .bg(Colors::background())
            .children([(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(
                |(mode, label)| {
                    BaseButton::new(SharedString::from(format!("mouse-effects-tab-{mode}")))
                        .selected(mode == advanced)
                        .disabled(!enabled)
                        .h(surface::css(26.))
                        .px(surface::css(10.))
                        .py(surface::css(5.))
                        .rounded(surface::css(13.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .bg(if mode == advanced {
                            Colors::accent()
                        } else {
                            Colors::transparent()
                        })
                        .text_color(if mode == advanced {
                            Colors::selected_text()
                        } else {
                            Colors::text()
                        })
                        .child(t(label))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(state) = &mut this.mouse_lighting {
                                state.ui_advanced = Some(mode);
                            }
                            this.dismiss_mouse_lighting(cx);
                            this.emit_lighting(
                                "TOGGLE_CHROMA",
                                json!({"isChromaEnabled":mode}),
                                cx,
                            );
                            if this.mouse_lighting_page_variant() == "babel_162" && !mode {
                                // OJ.enableEffectsEngine restores the selected
                                // profile's quickEffects when leaving Advanced.
                                this.emit_mouse_effect(cx);
                            }
                            this.sync_mouse_lighting(window, cx);
                            cx.notify();
                        }))
                },
            ));
        let mut body = v_flex().mt(surface::css(20.));
        if advanced {
            // Modern Xl waits for checkIsFullyInstall. Babel _J instead checks
            // commonReducer.installedModules and immediately shows Install
            // when the module is absent.
            let installed = if babel {
                Some(
                    observed["isChromaInstalled"] == true
                        || observed["installedModules"]
                            .as_array()
                            .is_some_and(|modules| {
                                modules.iter().any(|module| module == "chroma-app")
                            }),
                )
            } else {
                observed["allChromaResourcesInstalled"].as_bool()
            };
            if let Some(installed) = installed {
                if installed {
                    let profiles = observed["chromaProfiles"].as_array();
                    body = body
                        .child(t("ADVANCED_EFFECTS_MSG"))
                        .child(div().mt(surface::css(20.)).child(t(
                            if profiles.is_some_and(|profiles| !profiles.is_empty()) {
                                "CHROMA_STUDIO"
                            } else {
                                "NO_CHROMA_STUDIO_PROFILE_TEXT"
                            },
                        )))
                        .children(self.mouse_lighting.as_ref().map(|state| {
                            div().mt(surface::css(5.)).child(
                                surface::select(&state.chroma_profile)
                                    .items(self.chroma_choices())
                                    .disabled(!enabled)
                                    .accessibility_label(t("CHROMA_STUDIO")),
                            )
                        }))
                        .child(
                            Button::new("mouse-launch-chroma-studio")
                                .label(t("LAUNCH_CHROMA_STUDIO"))
                                .disabled(!enabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.emit_lighting(
                                        if babel {
                                            "openChromaStudio"
                                        } else {
                                            "launchChromaStudio"
                                        },
                                        json!({}),
                                        cx,
                                    )
                                })),
                        );
                } else {
                    body = body
                        .child(
                            div()
                                .relative()
                                .w(surface::css(520.))
                                .h(surface::css(180.))
                                .child(img("synapse/mouse-lighting/install-chroma.png").size_full())
                                .child(
                                    div()
                                        .absolute()
                                        .bottom(surface::css(20.))
                                        .w_full()
                                        .flex()
                                        .justify_center()
                                        .child(
                                            BaseButton::new("mouse-install-chroma")
                                                .flex()
                                                .items_center()
                                                .gap(surface::css(5.))
                                                .px(surface::css(15.))
                                                .py(surface::css(4.))
                                                .rounded(surface::css(2.))
                                                .border_1()
                                                .border_color(Colors::accent())
                                                .bg(Colors::accent())
                                                .text_color(Colors::selected_text())
                                                .text_size(surface::css(12.))
                                                .hover(|button| button.bg(Colors::install_hover()))
                                                .children(
                                                    (!babel
                                                        && observed["installationInProgress"]
                                                            == true)
                                                        .then_some(MouseChromaInstallSpinner),
                                                )
                                                .child(t("INSTALL_RAZER_CHROMA"))
                                                .disabled(
                                                    !enabled
                                                        || (!babel
                                                            && observed["installationInProgress"]
                                                                == true),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if babel {
                                                        this.emit_lighting(
                                                            "openDevicesModules",
                                                            json!({}),
                                                            cx,
                                                        );
                                                    } else {
                                                        this.emit_lighting(
                                                            "triggerInstall",
                                                            json!({"module":"chroma-app"}),
                                                            cx,
                                                        );
                                                    }
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .child(t("ADVANCED_EFFECTS_DETAIL")),
                        );
                }
            }
        } else {
            body = body
                .child(div().mb(surface::css(20.)).child(t(if hardware {
                    if self.mouse_lighting_config()["DeviceInfo"]["dongleId"]
                        .as_u64()
                        .is_some_and(|id| id != 0)
                    {
                        "BLE_QUICK_EFFECTS_MSG"
                    } else {
                        "BLE_NO_24GHZ_QUICK_EFFECTS_MSG"
                    }
                } else {
                    "QUICK_EFFECTS_MSG"
                })))
                .children(self.mouse_lighting.as_ref().map(|state| {
                    h_flex()
                        .items_center()
                        .gap(surface::css(20.))
                        .child(
                            div()
                                .w(surface::css(150.))
                                .flex_shrink_0()
                                .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                                    if this.check_mouse_adjustment_mode(cx) {
                                        cx.stop_propagation();
                                    }
                                }))
                                .child(
                                    surface::select(&state.effect)
                                        .items(self.rgb_choices())
                                        .disabled(!self.mouse_quick_enabled())
                                        .accessibility_label(t("QUICK_EFFECTS"))
                                        .w_full(),
                                ),
                        )
                        .when(!hardware, |row| {
                            row.child(
                                BaseButton::new("mouse-chroma-sync")
                                    .disabled(!self.can_sync_mouse_effect())
                                    .opacity(if self.can_sync_mouse_effect() { 1. } else { 0.3 })
                                    .flex()
                                    .items_center()
                                    .child(self.mouse_sync_icon())
                                    .child(div().underline().child(t(
                                        if self.can_sync_mouse_effect() {
                                            "APPLY_TO_ALL_CHROMA_DEVICES"
                                        } else if self.rgb_effect_id() == 17 {
                                            "NOT_APPLICABLE_CHROMA_ENABLED_DEVICES"
                                        } else {
                                            "ONLY_ONE_CHROMA_DEVICE"
                                        },
                                    )))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.sync_mouse_effect(cx)),
                                    ),
                            )
                        })
                }))
                .child(
                    div()
                        .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                            if this.check_mouse_adjustment_mode(cx) {
                                cx.stop_propagation();
                            }
                        }))
                        .child(self.rgb_parameters(cx)),
                );
        }
        let _ = window;
        surface::panel_with_help(
            t("EFFECTS"),
            surface::help_control(
                "mouse-effects-help",
                t(if hardware {
                    "EFFECTS_BLE_TOOLTIP"
                } else {
                    "EFFECTS_TOOLTIP"
                }),
            ),
            cx,
        )
        .when(!hardware, |panel| panel.child(tabs))
        .child(body)
        .children(
            (observed["isChromaVisualizerEnabled"] == true).then(|| self.mouse_chroma_status(cx)),
        )
        .into_any_element()
    }

    fn mouse_basic_effects(&self, cx: &Context<Self>) -> AnyElement {
        let is_static = self.draft["quickEffects"]["selectedEffectId"] == 1;
        surface::panel_with_help(t("EFFECTS"),surface::help_control("mouse-basic-effects-help",t("EFFECTS_TOOLTIP_RADIO")),cx)
            .child(v_flex().h(surface::css(50.)).justify_between()
                .children([(false,"BREATHING","CheckBreathing"),(true,"STATIC","CheckStatic")].map(|(value,label,id)| {
                    gpui_kit::component::radio::Radio::new(id).label(t(label)).checked(value==is_static)
                        .on_click(cx.listener(move |this,_,_,cx| {
                            if (this.draft["quickEffects"]["selectedEffectId"] == 1) == value {return;}
                            let id=if value {1} else {2};
                            let setting=if value {json!({"color1":"#00ff00"})} else {json!({"color1":"#00ff00","color2":"no-color","isRandom":false})};
                            this.store_rgb_setting(id,setting.clone());
                            set_pointer(&mut this.draft,"/quickEffects/selectedEffectSetting",setting.clone());
                            this.emit_lighting("ON_SET_EFFECTS",json!({"effectSetting":{"selectedEffectId":id,"selectedEffectSetting":setting,"regionId":0}}),cx);
                            cx.emit(MouseProductChanged);cx.notify();
                        }))
                }))).into_any_element()
    }

    fn mouse_chroma_status(&self, cx: &Context<Self>) -> AnyElement {
        let app = &self.lighting_observation()["chromaConnectApp"];
        let icon = if app["appName"] == "Chroma Visualizer App" {
            Some("synapse/mouse-lighting/logo-visualizer.svg")
        } else {
            app["appIcon"].as_str().filter(|icon| !icon.is_empty())
        };
        let name = app["appName"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .split(' ')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().to_string() + chars.as_str()
                })
            })
            .collect::<Vec<_>>()
            .join(" ")
            .replace(".exe", "");
        h_flex()
            .relative()
            .w_full()
            .h(surface::css(45.))
            .mt(surface::css(20.))
            .rounded(surface::css(5.))
            .bg(cx.theme().group_box)
            .children(icon.map(|icon| {
                img(SharedString::from(icon.to_owned()))
                    .size(surface::css(25.))
                    .m(surface::css(10.))
            }))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}:", t("APP_IN_USE"))),
                    )
                    .child(format!("{name} ({})", t("CHROMA_APPS"))),
            )
            .child(
                BaseButton::new("mouse-manage-chroma-apps")
                    .accessibility_label(t("MANAGE_CHROMA_APPS"))
                    .size(surface::css(20.))
                    .mr(surface::css(10.))
                    .child(img("synapse/mouse-lighting/settings.svg").size(surface::css(16.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.mouse_lighting_page_variant() == "babel_162" {
                            this.emit_lighting("focusChromaAppsBabel", json!({}), cx);
                            return;
                        }
                        let install_required =
                            this.lighting_observation()["allChromaResourcesInstalled"] != true;
                        this.emit_lighting(
                            "focusChromaApps",
                            json!({"installRequired":install_required}),
                            cx,
                        );
                    })),
            )
            .child(surface::help_control(
                "mouse-chroma-status-help",
                format!(
                    "{}.\n\n{}.",
                    t("CHROMA_INDICATOR_TIP_1"),
                    t("CHROMA_INDICATOR_TIP_2")
                ),
            ))
            .into_any_element()
    }

    fn select_mouse_effect(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.check_mouse_adjustment_mode(cx) {
            return;
        }
        if !self.mouse_quick_enabled() {
            return;
        }
        let setting = self.draft["quickEffects"]["effectSettings"]
            .as_array()
            .and_then(|settings| settings.iter().find(|setting| setting["effectId"] == id))
            .cloned()
            .unwrap_or_else(|| self.default_rgb_setting(id));
        self.store_rgb_setting(id, setting);
        self.sync_mouse_lighting(window, cx);
        self.emit_mouse_effect(cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    pub(super) fn init_mouse_lighting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !lighting_supported(self.spec.product_id) {
            return;
        }
        let generation = self
            .mouse_lighting
            .as_ref()
            .map_or(0, |state| state.generation.wrapping_add(1));
        let effect = cx.new(|cx| SelectState::new(self.rgb_choices(), None, window, cx));
        let chroma_profile = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &chroma_profile,
            window,
            |this, _, event, _, cx| {
                // _J.render passes an undefined this.changeEffect. Its constructor,
                // prototype and connector define no profile submission callback.
                // Preserve the unresolved source action instead of borrowing Xl's.
                if this.mouse_lighting_page_variant() == "babel_162" {
                    return;
                }
                if this.syncing
                    || this.lighting_observation()["nanoLeafEnabled"] == false
                    || this.lighting_observation()["isChromaVisualizerEnabled"] == true
                {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    let selected = this.lighting_observation()["chromaProfiles"]
                        .as_array()
                        .and_then(|profiles| {
                            profiles
                                .iter()
                                .find(|profile| profile["id"].as_str() == Some(id.as_str()))
                        })
                        .map(|profile| profile["id"].clone());
                    if let Some(id) = selected {
                        if let Some(state) = &mut this.mouse_lighting {
                            state.ui_chroma_profile = id.as_str().map(str::to_owned);
                        }
                        this.emit_lighting(
                            "ON_SET_CHROMA_PROFILE",
                            json!({"selectedChromaProfileGuid":id}),
                            cx,
                        );
                        cx.notify();
                    }
                }
            },
        ));
        let colors = std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx)));
        let power_indicator_color = cx.new(|cx| ColorPickerState::new(window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &power_indicator_color,
            window,
            |this, _, event, _, cx| {
                if this.syncing {
                    return;
                }
                let ColorPickerEvent::Change(Some(color)) = event else {
                    return;
                };
                let c = Rgba::from(*color);
                this.set_mouse_power_indicator(
                    "color",
                    json!(format!(
                        "#{:02x}{:02x}{:02x}",
                        (c.r * 255.).round() as u8,
                        (c.g * 255.).round() as u8,
                        (c.b * 255.).round() as u8
                    )),
                    cx,
                );
            },
        ));
        let duration = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(3.)
                .step(1.)
                .default_value(2.)
        });
        let boost = cx.new(|cx| {
            Stepper::new(
                "mouse-effect-boost",
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
        });
        for (channel, color) in colors.iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                color,
                window,
                move |this, _, event, window, cx| {
                    let id = this.rgb_effect_id();
                    let setting = this.rgb_setting();
                    if this.syncing
                        || !this.mouse_quick_enabled()
                        || !matches!(id, 1 | 2 | 5 | 6 | 7 | 17 | 19)
                        || (id != 17 && setting["isRandom"] == true)
                        || (id == 17 && setting["type"] == "gradient pattern")
                    {
                        return;
                    }
                    let ColorPickerEvent::Change(color) = event;
                    if color.is_none() && matches!(id, 1 | 5 | 6 | 17) {
                        return;
                    }
                    let color = color
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
                    let key = if id == 17 {
                        "color"
                    } else if channel == 0 {
                        "color1"
                    } else {
                        "color2"
                    };
                    this.change_rgb_setting(key, json!(color), window, cx);
                },
            ));
        }
        self.subscriptions.push(cx.subscribe_in(
            &duration,
            window,
            |this, _, event, window, cx| {
                if !this.syncing
                    && this.mouse_quick_enabled()
                    && matches!(this.rgb_effect_id(), 5 | 7)
                {
                    if let SliderEvent::Release(value) = event {
                        this.change_rgb_setting(
                            "duration",
                            json!(value.start().round().clamp(1., 3.) as u64),
                            window,
                            cx,
                        );
                    }
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &boost,
            window,
            |this, _, event: &StepperEvent, window, cx| {
                if !this.syncing
                    && this.mouse_quick_enabled()
                    && this.rgb_effect_id() == 12
                    && event.value.is_finite()
                {
                    this.change_rgb_setting(
                        "colorBoost",
                        json!((event.value.clamp(0.25, 4.) * 4.).ceil() / 4.),
                        window,
                        cx,
                    );
                }
            },
        ));
        self.subscriptions.push(
            cx.subscribe_in(&effect, window, |this, _, event, window, cx| {
                if this.syncing || !this.mouse_quick_enabled() {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Ok(id) = value.parse::<u64>() {
                        if this
                            .rgb_effect_list()
                            .is_some_and(|effects| effects.iter().any(|effect| effect["id"] == id))
                        {
                            this.select_mouse_effect(id, window, cx);
                        }
                    }
                }
            }),
        );
        self.mouse_lighting = Some(State {
            observation: json!({}),
            cached_effect_settings: BTreeMap::new(),
            cached_restore_requested: false,
            ui_advanced: None,
            ui_chroma_profile: None,
            generation,
            request_id: 0,
            custom_colors_draft: None,
            sync_started: None,
            sync_animation: 0,
            pending: BTreeMap::new(),
            latest: BTreeMap::new(),
            error: None,
            effect,
            chroma_profile,
            colors,
            power_indicator_color,
            duration,
            boost,
        });
        self.sync_mouse_lighting(window, cx);
    }

    pub(super) fn rgb_effect_id(&self) -> u64 {
        self.draft["quickEffects"]["selectedEffectId"]
            .as_u64()
            .unwrap_or(3)
    }

    pub(super) fn rgb_hardware_effect(&self) -> bool {
        self.lighting_observation()["isBle"] == true
            && self.mouse_lighting_config()["DeviceInfo"]["ble"]["useHardwareEffect"] == true
    }

    pub(super) fn rgb_effect_list(&self) -> Option<&Vec<Value>> {
        if let Some(effects) = self.mouse_lighting_config()["QUICK_EFFECTS_SPECIAL"]
            .as_array()
            .and_then(|editions| {
                editions
                    .iter()
                    .find(|edition| edition["EID"] == self.lighting_observation()["editionId"])
            })
            .and_then(|edition| edition["value"].as_array())
        {
            return Some(effects);
        }
        self.mouse_lighting_config()[if self.rgb_hardware_effect() {
            "HARDWARE_QUICK_EFFECTS"
        } else {
            "QUICK_EFFECTS"
        }]
        .as_array()
    }

    pub(super) fn rgb_choices(&self) -> Vec<Choice> {
        self.rgb_effect_list()
            .into_iter()
            .flatten()
            .filter_map(|effect| {
                Some(Choice::localized(
                    effect["id"].as_u64()?.to_string(),
                    effect["name"].as_str()?,
                ))
            })
            .collect()
    }

    pub(super) fn default_rgb_setting(&self, id: u64) -> Value {
        let mut setting = self.mouse_lighting_config()["DEFAULT_CUSTOM_EFFECT_SETTINGS"]
            .get(id.to_string())
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| {
                self.mouse_lighting_config()["DEFAULT_EFFECT_SETTINGS"]
                    .get(id.to_string())
                    .cloned()
                    .unwrap_or_else(|| json!({}))
            });
        if id == 4 {
            setting["direction"] = json!(
                match self.mouse_lighting_config()["DeviceInfo"]["WAVE_DIRECTION"].as_str() {
                    Some("CWCCW") => 12,
                    Some("UpDown") => 4,
                    _ => 2,
                }
            );
        }
        if id == 19 {
            if let Some(directions) = self.mouse_lighting_config()["DeviceInfo"]["TIDAL_DIRECTION"]
                .as_array()
                .filter(|directions| directions.len() == 2)
            {
                setting["direction"] = directions[0]["value"].clone();
            }
        }
        setting
    }

    fn chroma_choices(&self) -> Vec<Choice> {
        self.lighting_observation()["chromaProfiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|profile| {
                Some(Choice::new(
                    profile["id"].as_str()?,
                    profile["name"].as_str()?.to_owned(),
                ))
            })
            .collect()
    }

    pub(super) fn rgb_setting(&self) -> Value {
        let id = self.rgb_effect_id();
        self.draft["quickEffects"]["effectSettings"]
            .as_array()
            .and_then(|settings| settings.iter().find(|setting| setting["effectId"] == id))
            .or_else(|| self.draft["quickEffects"].get("selectedEffectSetting"))
            .cloned()
            .unwrap_or_else(|| self.default_rgb_setting(id))
    }

    pub(super) fn sync_mouse_lighting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.mouse_lighting else {
            return;
        };
        let setting = self.rgb_setting();
        let id = self.rgb_effect_id();
        let choices = self.rgb_choices();
        let chroma_choices = self.chroma_choices();
        let chroma_selected = state.ui_chroma_profile.clone().or_else(|| {
            self.lighting_observation()["selectedChromaProfileGuid"]
                .as_str()
                .map(str::to_owned)
        });
        let was_syncing = self.syncing;
        self.syncing = true;
        for path in ["/brightness/value", "/switchOffLighting/idleMinutes"] {
            let value = self.number(path);
            if let Some(slider) = self.sliders.get(path) {
                slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
            }
            if let Some(input) = self.inputs.get(path) {
                input.update(cx, |input, cx| {
                    input.set_value((value as i64).to_string(), window, cx)
                });
            }
        }
        if let Some(color) = self.draft["powerIndicator"]["color"]
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|s| s.len() == 6)
            .and_then(|s| u32::from_str_radix(s, 16).ok())
        {
            state
                .power_indicator_color
                .update(cx, |picker, cx| picker.set_value(rgb(color), window, cx));
        }
        state.effect.update(cx, |effect, cx| {
            effect.set_items(choices, window, cx);
            effect.set_selected_value(&id.to_string(), window, cx)
        });
        state.chroma_profile.update(cx, |profile, cx| {
            profile.set_items(chroma_choices, window, cx);
            if let Some(selected) = chroma_selected {
                profile.set_selected_value(&selected, window, cx);
            }
        });
        for (channel, color) in state.colors.iter().enumerate() {
            let key = if id == 17 {
                "color"
            } else if channel == 0 {
                "color1"
            } else {
                "color2"
            };
            let value = setting[key]
                .as_str()
                .and_then(|s| s.strip_prefix('#'))
                .filter(|s| s.len() == 6)
                .and_then(|s| u32::from_str_radix(s, 16).ok());
            color.update(cx, |color, cx| {
                if let Some(value) = value {
                    color.set_value(rgb(value), window, cx);
                } else {
                    color.clear_value(window, cx);
                }
            });
        }
        state.duration.update(cx, |slider, cx| {
            slider.set_value(
                setting["duration"].as_f64().unwrap_or(2.) as f32,
                window,
                cx,
            )
        });
        state.boost.update(cx, |boost, cx| {
            boost.sync_value(
                setting["colorBoost"].as_f64().unwrap_or(1.),
                0.25,
                id != 12 || !self.mouse_quick_enabled(),
                window,
                cx,
            )
        });
        self.syncing = was_syncing;
    }

    pub(super) fn dismiss_mouse_lighting(&self, cx: &mut Context<Self>) {
        if let Some(state) = &self.mouse_lighting {
            state
                .power_indicator_color
                .update(cx, |picker, cx| picker.set_open(false, cx));
            for color in &state.colors {
                color.update(cx, |color, cx| color.set_open(false, cx));
            }
        }
    }

    fn change_rgb_setting(
        &mut self,
        key: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.check_mouse_adjustment_mode(cx) {
            return;
        }
        if !self.mouse_quick_enabled() {
            return;
        }
        if !self.mouse_quick_enabled() {
            return;
        }
        let id = self.rgb_effect_id();
        // Static and the desktop Audio Meter component replace their setting;
        // the other mounted children spread the old setting before the patch.
        let mut setting = if id == 1 || (id == 12 && key == "colorBoost") {
            json!({})
        } else {
            self.rgb_setting()
        };
        setting[key] = value;
        self.store_rgb_setting(id, setting);
        self.sync_mouse_lighting(window, cx);
        self.emit_mouse_effect(cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }

    pub(super) fn store_rgb_setting(&mut self, id: u64, mut setting: Value) {
        if let Some(object) = setting.as_object_mut() {
            object.remove("effectId");
        }
        self.draft["quickEffects"]["selectedEffectId"] = json!(id);
        self.draft["quickEffects"]["selectedEffectSetting"] = setting.clone();
        let Some(effects) = self.draft["quickEffects"].as_object_mut() else {
            return;
        };
        let settings = effects.entry("effectSettings").or_insert_with(|| json!([]));
        let Some(settings) = settings.as_array_mut() else {
            return;
        };
        settings.retain(|setting| setting["effectId"] != id);
        setting["effectId"] = json!(id);
        settings.push(setting);
    }

    pub(super) fn rgb_parameters(&self, cx: &Context<Self>) -> AnyElement {
        let Some(state) = &self.mouse_lighting else {
            return div().into_any_element();
        };
        let id = self.rgb_effect_id();
        let setting = self.rgb_setting();
        match id {
            1 | 2 | 5 | 6 | 7 | 17 | 19 => {
                let multiple = matches!(id, 2 | 7 | 19);
                let random = setting["isRandom"] == true;
                let analog_v2 =
                    self.mouse_lighting_config()["DeviceInfo"]["AnalogGenVersion"] == "analogV2";
                let gradient = setting["type"] == "gradient pattern";
                let mut controls = h_flex().items_start().flex_wrap();
                for channel in 0..if multiple { 2 } else { 1 } {
                    let label = if multiple {
                        t("COLOR_DROP_NAME").replace("{{num}}", &(channel + 1).to_string())
                    } else {
                        t("COLOR")
                    };
                    controls = controls.child(
                        v_flex()
                            .mt(surface::css(20.))
                            .mr(surface::css(20.))
                            .child(label.clone())
                            .child(
                                div().mt(surface::css(5.)).child(
                                    LightingColorPicker::new(&state.colors[channel], label)
                                        .allow_none(multiple)
                                        .on_custom_colors_commit(cx.listener(
                                            |this,
                                             colors: &razer_state::CustomColorSlots,
                                             _,
                                             cx| {
                                                this.commit_mouse_custom_colors(colors, cx)
                                            },
                                        ))
                                        .disabled(
                                            !self.mouse_quick_enabled()
                                                || if id == 17 { gradient } else { random },
                                        ),
                                ),
                            ),
                    );
                }
                if multiple || id == 17 {
                    controls = controls.child(
                        div().mt(surface::css(45.)).child(
                            Checkbox::new("mouse-effect-random")
                                .disabled(!self.mouse_quick_enabled())
                                .label(t(if id == 17 && analog_v2 {
                                    "GRADIENT_PATTERN"
                                } else {
                                    "RANDOM_COLOR"
                                }))
                                .checked(if id == 17 && analog_v2 { gradient } else { random })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if id == 17 && analog_v2 {
                                        this.change_rgb_setting(
                                            "type",
                                            json!(if gradient {
                                                "static"
                                            } else {
                                                "gradient pattern"
                                            }),
                                            window,
                                            cx,
                                        );
                                    } else {
                                        this.change_rgb_setting(
                                            "isRandom",
                                            json!(!random),
                                            window,
                                            cx,
                                        );
                                    }
                                })),
                        ),
                    );
                }
                if matches!(id, 5 | 7) {
                    controls = controls.child(self.rgb_duration());
                }
                v_flex()
                    .child(controls)
                    .when(id == 19, |view| view.child(self.rgb_direction(19, cx)))
                    .into_any_element()
            }
            4 | 13 => self.rgb_direction(id, cx),
            11 => self.rgb_screen(cx),
            12 => v_flex()
                .mt(surface::css(20.))
                .child(t("TEXT_COLOR_BOOST"))
                .child(div().mt(surface::css(10.)).child(state.boost.clone()))
                .into_any_element(),
            // Spectrum and Fire render no parameter controls in these callers.
            _ => div().into_any_element(),
        }
    }

    fn rgb_duration(&self) -> AnyElement {
        let state = self.mouse_lighting.as_ref().expect("RGB controls");
        v_flex()
            .mt(surface::css(20.))
            .ml(surface::css(10.))
            .when(self.rgb_effect_id() == 7, |control| {
                control.w(surface::css(230.))
            })
            .when(self.rgb_effect_id() == 5, |control| control.flex_1())
            .child(t("TEXT_DURATION"))
            .child(
                SourceSlider::new(
                    &state.duration,
                    ((self.rgb_setting()["duration"].as_f64().unwrap_or(2.) - 1.) / 2.) as f32,
                )
                .enabled(self.mouse_quick_enabled()),
            )
            .child(surface::slider_tags(
                &t("TEXT_SLIDER_SHORT"),
                Some(&t("TEXT_SLIDER_MEDIUM")),
                &t("TEXT_SLIDER_LONG"),
                None,
            ))
            .into_any_element()
    }

    fn rgb_direction(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let setting = self.rgb_setting();
        let direction = setting["direction"].as_u64();
        let info = &self.mouse_lighting_config()["DeviceInfo"];
        let explicit = info[if id == 19 {
            "TIDAL_DIRECTION"
        } else {
            "WAVE_DIRECTION"
        }]
        .as_array();
        let choices: Vec<(u64, String)> =
            if let Some(values) = explicit.filter(|values| id != 19 || values.len() == 2) {
                values
                    .iter()
                    .filter_map(|value| {
                        Some((
                            value["value"].as_u64()?,
                            value["id"].as_str()?.trim_start_matches("dir-").to_owned(),
                        ))
                    })
                    .collect()
            } else {
                let values = match id {
                    13 => [(1, "cw"), (2, "ccw")],
                    19 => [(1, "out"), (0, "in")],
                    _ if info["WAVE_DIRECTION"] == "CWCCW" => [(11, "cw"), (12, "ccw")],
                    _ if info["WAVE_DIRECTION"] == "UpDown" => [(3, "up"), (4, "down")],
                    _ => [(1, "left"), (2, "right")],
                };
                values
                    .into_iter()
                    .map(|(value, name)| (value, name.to_owned()))
                    .collect()
            };
        v_flex()
            .mt(surface::css(20.))
            .child(t("TEXT_DIRECTION"))
            .child(
                h_flex()
                    .mt(surface::css(5.))
                    .border_1()
                    .border_color(Colors::border())
                    .rounded(surface::css(3.))
                    .children(choices.into_iter().map(|(value, name)| {
                        let selected = direction == Some(value);
                        BaseButton::new(SharedString::from(format!(
                            "mouse-effect-direction-{name}"
                        )))
                        .disabled(!self.mouse_quick_enabled())
                        .w(surface::css(51.))
                        .h(surface::css(25.))
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
                                "synapse/{}direction-{name}{}.svg",
                                if matches!(name.as_str(), "up" | "down") {
                                    "mouse-lighting/"
                                } else {
                                    ""
                                },
                                if selected { "-active" } else { "" }
                            )))
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.change_rgb_setting("direction", json!(value), window, cx)
                            },
                        ))
                    })),
            )
            .into_any_element()
    }

    fn rgb_screen(&self, cx: &Context<Self>) -> AnyElement {
        let setting = self.rgb_setting();
        v_flex()
            .relative()
            .mt(surface::css(20.))
            .h(surface::css(60.))
            .child(t("TEXT_SCREEN_REGION"))
            .children(
                ["full", "left", "top", "right", "bottom"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, region)| {
                        let selected = setting["screen"] == region;
                        BaseButton::new(SharedString::from(format!("mouse-effect-screen-{region}")))
                            .absolute()
                            .top(surface::css(40.))
                            .left(surface::css(index as f32 * 30.))
                            .w(surface::css(24.))
                            .h(surface::css(16.))
                            .p_0()
                            .border(surface::css(if selected { 1.5 } else { 1. }))
                            .border_color(if selected {
                                Colors::accent()
                            } else {
                                Colors::screen_border()
                            })
                            .when(region == "full", |screen| {
                                screen.bg(KeyboardEffectColors::white())
                            })
                            .when(region != "full", |screen| {
                                screen.child(
                                    div()
                                        .absolute()
                                        .bg(KeyboardEffectColors::white())
                                        .when(matches!(region, "left" | "right"), |stripe| {
                                            stripe.w(surface::css(4.)).h_full()
                                        })
                                        .when(matches!(region, "top" | "bottom"), |stripe| {
                                            stripe.h(surface::css(4.)).w_full()
                                        })
                                        .when(region == "right", |stripe| stripe.right_0())
                                        .when(region == "bottom", |stripe| stripe.bottom_0())
                                        .when(matches!(region, "top" | "left"), |stripe| {
                                            stripe.top_0().left_0()
                                        }),
                                )
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_rgb_setting("screen", json!(region), window, cx)
                            }))
                    }),
            )
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/mouse_lighting_current_data.
fn read_mouse_lighting_current_catalog() -> Vec<SourceLighting> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SourceLighting>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceLighting> {
        let value = match r.tag()? {
            0 => SourceLighting {
                product_id: r.child(decode_2)?,
                shared_quick_renderer: r.child(decode_3)?,
                page_variant: r.child(decode_4)?,
                config: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_LIGHTING_CURRENT_DATA,
        0,
        "c71f3e76ba24b1dbdea55f598983e38585ebc43ce87d666df87973e5312703c3",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_lighting_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_lighting_current_data_decodes() {
        let _ = super::read_mouse_lighting_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_lighting_current_data.
