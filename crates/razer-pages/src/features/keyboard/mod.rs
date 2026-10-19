//! Native drafts for current keyboard and keypad product sources.
//! Specifications are statically extracted; no vendor JavaScript is executed.
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, StyledExt,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n::t;
use razer_state::ui_preferences::{Flag as UiPreference, UiPreferences};
use razer_widgets::surface;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[path = "actuation/keyboard_actuation.rs"]
mod actuation;
#[path = "actuation/keyboard_actuation_submission.rs"]
mod actuation_submission;
#[path = "actuation/keyboard_analog_observation.rs"]
mod analog_observation;
#[path = "actuation/keyboard_gamepad_observation.rs"]
mod gamepad_observation;
#[path = "actuation/keyboard_observation_tasks.rs"]
mod observation_tasks;
pub use actuation_submission::KeyboardActuationRequested;
#[path = "lighting/keyboard_brightness.rs"]
mod brightness;
#[path = "mapping/keyboard_mapping_runtime_515.rs"]
mod legacy_mapping;
pub use legacy_mapping::{KeyboardLegacyProfileNavigation, KeyboardLegacyRuntimeStatus};
#[path = "lighting/keyboard_chroma_status.rs"]
mod chroma_status;
#[path = "keyboard_customize_gates.rs"]
mod customize_gates;
#[path = "keyboard_keymap_bar.rs"]
mod keymap_bar;
#[path = "keyboard_layout.rs"]
mod layout;
#[path = "mapping/keyboard_mapping_login_warning.rs"]
mod mapping_login_warning;
pub use mapping_login_warning::{
    KeyboardMappingLoginWarningHidden, KeyboardMappingLoginWarningReadRequested,
};
#[path = "lighting/keyboard_lighting_effects.rs"]
mod lighting_effects;
#[path = "lighting/keyboard_lighting_gates.rs"]
mod lighting_gates;
#[path = "lighting/keyboard_lighting_runtime.rs"]
mod lighting_runtime;
#[path = "lighting/keyboard_rgb_controls.rs"]
mod rgb_controls;
#[path = "lighting/keyboard_rgb_effects.rs"]
mod rgb_effects;
pub use lighting_runtime::{
    KeyboardChromaPeer, KeyboardLightingCommand, KeyboardLightingHostObservation,
    KeyboardLightingObservation, KeyboardLightingOwner, KeyboardLightingRequested,
};
#[path = "calibration/keyboard_calibration.rs"]
mod calibration;
#[path = "calibration/keyboard_calibration_v2.rs"]
mod calibration_v2;
pub use calibration_v2::KeyboardNavigationRequested;
#[path = "gaming/keyboard_gaming_combined.rs"]
mod gaming_combined;
#[path = "gaming/keyboard_gaming_rows.rs"]
mod gaming_rows;
#[path = "keyboard_polling.rs"]
mod polling;
#[path = "keyboard_power.rs"]
mod power;
pub use polling::KeyboardPollingConnection;
pub use power::{KeyboardIndicatorLedObservation, KeyboardIndicatorLedRequested};
#[path = "actuation/keyboard_analog_controls.rs"]
mod analog_controls;
#[path = "actuation/keyboard_analog_gamepad.rs"]
mod analog_gamepad;
pub use analog_controls::KeyboardAnalogControlsObservation;
#[path = "controller/keyboard_floating_controller.rs"]
mod floating_controller;
#[path = "artwork/keyboard_source_artwork.rs"]
mod source_artwork;
pub use analog_gamepad::source_keypad_product_image;
pub use source_artwork::{
    SourceArtworkObservation, source_preview_image_for_scale, source_preview_layouts,
    source_preview_layouts_for_edition,
};
#[path = "keyboard_properties.rs"]
mod properties;
pub use analog_gamepad::KeyboardGamepadTesterObservation;
pub use analog_gamepad::KeyboardKeypadThumbstickObservation;
pub use analog_gamepad::KeyboardMappingAiObservation;
pub use analog_gamepad::KeyboardMappingInterDeviceConfigureRequested;
pub use analog_gamepad::KeyboardMappingInterDeviceObservation;
pub use analog_gamepad::KeyboardMappingMacroObservation;
pub use analog_gamepad::KeyboardMappingProfilesObservation;
pub use analog_gamepad::{KeyboardMappingDialObservation, KeyboardMappingDialReadRequested};
pub use analog_gamepad::{
    KeyboardMappingSensitivityWarningHidden, KeyboardMappingSensitivityWarningReadRequested,
};
pub use brightness::KeyboardBrightnessReadRequested;
pub use brightness::KeyboardBrightnessRequested;
#[path = "actuation/keyboard_snap_tap.rs"]
mod snap_tap;
pub use calibration::open_preview as open_calibration_preview;
pub use calibration::{KeyboardLegacyCalibrationHostRequested, LegacyCalibrationHostCommand};
pub(super) use customize_gates::is_factory_profile;
pub use snap_tap::SnapTapObservation;

#[derive(Deserialize)]
pub struct KeyboardProductSpec {
    product_id: u32,
    #[serde(default)]
    source_layout: Option<String>,
    #[serde(default)]
    source_mod_tap: bool,
    #[serde(default)]
    source_keyboard_top_padding_percent: f32,
    #[serde(default)]
    source_keyboard_top_padding_px: f32,
    name: String,
    config: Value,
    pages: Vec<String>,
    keys: Vec<Value>,
    shapes: Vec<razer_assets::KeyboardKey>,
    viewbox: [f32; 2],
    image: Option<String>,
    image_size: [f32; 2],
    controls: Value,
}
impl KeyboardProductSpec {
    fn analog_gamepad_layout(&self) -> bool {
        self.source_layout.as_deref() == Some("analog_gamepad")
    }
    pub fn macro_keys(&self) -> &[Value] {
        &self.keys
    }
    pub fn macro_shapes(&self) -> &[razer_assets::KeyboardKey] {
        &self.shapes
    }
    pub fn macro_image(&self) -> Option<&str> {
        self.image.as_deref()
    }
    pub fn macro_viewbox(&self) -> [f32; 2] {
        self.viewbox
    }
    fn default_profile(&self) -> Value {
        let mut profile = self.config["DEFAULTPROFILE"].clone();
        if let (Some(profile), Some(defaults)) = (
            profile.as_object_mut(),
            self.controls["defaults"].as_object(),
        ) {
            for (key, value) in defaults {
                profile.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        profile
    }
}
pub fn source_product(pid: u32) -> Option<&'static KeyboardProductSpec> {
    static PRODUCTS: super::source_catalog::SourceCatalog<KeyboardProductSpec> =
        super::source_catalog::SourceCatalog::new(|| read_keyboard_products_catalog());
    PRODUCTS.get(pid)
}
pub(crate) fn mapping_target_info(product_id: u32) -> Option<&'static Value> {
    source_product(product_id).map(|source| &source.config["DeviceInfo"])
}
pub struct KeyboardProductChanged;

pub struct KeyboardCharacterMapRequested;
impl EventEmitter<KeyboardCharacterMapRequested> for KeyboardProductWorkspace {}
/// `kI` 的两个勾选项：状态路径与文案键（源码别名 `Rt.ISS`/`Rt.CUC` 解析为
/// `DISPLAY_TURNED_OFF`、`IDLE_FOR_MIN`）。
pub const SWITCH_OFF_LIGHTING_ITEMS: [(&str, &str); 2] = [
    ("/switchOffLighting/isDisplayOn", "DISPLAY_TURNED_OFF"),
    ("/switchOffLighting/isIdleEnabled", "IDLE_FOR_MIN"),
];

/// 空闲滑条的量程：`min:1`、`max:15`、`step:1`、`minTag:"1"`、`maxTag:"15"`。
pub const SWITCH_OFF_IDLE_MINUTES: (i64, i64) = (1, 15);

#[cfg(test)]
#[path = "keyboard_products_tests.rs"]
mod tests;

pub struct KeyboardProductWorkspace {
    spec: &'static KeyboardProductSpec,
    page: String,
    draft: Value,
    selected_key: Option<String>,
    hovered_key: Option<String>,
    hypershift: bool,
    sliders: BTreeMap<String, Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    actuation: Option<actuation::State>,
    actuation_submission: actuation_submission::State,
    ui_preferences: Entity<UiPreferences>,
    /// Source `profileReducer.isFactoryDefaultProfile`, shared by Customize,
    /// Actuation, Lighting and Calibration according to their mounted branches.
    factory_default_profile: bool,
    /// Explicit development workspace only; live calibration never fabricates events.
    calibration_preview: bool,
    calibrated_keys: Vec<Value>,
    calibration_modal: Option<Entity<calibration::CalibrationModal>>,
    calibration_v2: calibration_v2::State,
    snap_tap: Option<snap_tap::State>,
    snap_observer: snap_tap::observation::Observer,
    analog_observer: analog_observation::Observer,
    gamepad_observer: gamepad_observation::Observer,
    /// Retained OS icon choice; failed queries fall back to the legacy icon.
    properties_icon: Option<&'static str>,
    brightness: brightness::State,
    polling_connection: Option<KeyboardPollingConnection>,
    rgb_controls: Option<rgb_controls::State>,
    lighting_observation: Option<KeyboardLightingObservation>,
    lighting_runtime: lighting_runtime::State,
    power_indicator: power::IndicatorState,
    /// Audited Analog Gamepad callers keep the drawer independent of the mapping popup.
    button_drawer_open: bool,
    analog_gamepad: analog_gamepad::State,
    analog_controls: analog_controls::State,
    gaming_combined: gaming_combined::State,
    dual_link_widget: Option<Entity<super::dock_pairing::DualLinkWidget>>,
    source_artwork: source_artwork::ArtworkState,
    keymap_bar: keymap_bar::State,
    mapping_login_warning: mapping_login_warning::State,
    mapping_capabilities: Result<razer_model::mapping_requirements::MappingCapabilities, String>,
    legacy_mapping: legacy_mapping::State,
}

#[derive(IntoElement)]
struct SourceBaseArtwork {
    observation: SourceArtworkObservation,
    customize: bool,
    image_left: f32,
    image_top: f32,
    image_size: [f32; 2],
}
impl SourceBaseArtwork {
    fn new(observation: SourceArtworkObservation, frame: &layout::KeyboardImageFrame) -> Self {
        Self {
            observation,
            customize: frame.customize_artwork,
            image_left: frame.image_origin[0],
            image_top: frame.image_origin[1],
            image_size: frame.image_size,
        }
    }

    fn artwork_for_scale(&self, scale: f32) -> Option<source_artwork::ArtworkImage> {
        if self.customize {
            source_artwork::customize_image_for_scale(self.observation, scale)
        } else {
            source_artwork::base_image_for_scale(self.observation, scale)
        }
    }
}
impl RenderOnce for SourceBaseArtwork {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        self.artwork_for_scale(window.scale_factor())
            .map(|artwork| {
                let [width, height] = self.image_size;
                img(artwork.asset)
                    .absolute()
                    .left(surface::css(self.image_left))
                    .top(surface::css(self.image_top))
                    .w(surface::css(width))
                    .h(surface::css(height))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element())
    }
}
impl EventEmitter<KeyboardProductChanged> for KeyboardProductWorkspace {}
impl KeyboardProductWorkspace {
    pub fn new(
        pid: u32,
        factory_default_profile: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let spec = source_product(pid).expect("audited keyboard product");
        let ui_preferences = UiPreferences::shared(cx);
        let mut this = Self {
            spec,
            page: "TAB_CUSTOMIZE".into(),
            draft: spec.default_profile(),
            selected_key: None,
            hovered_key: None,
            hypershift: false,
            sliders: BTreeMap::new(),
            subscriptions: vec![],
            syncing: false,
            actuation: None,
            actuation_submission: actuation_submission::State::default(),
            ui_preferences,
            factory_default_profile,
            calibration_preview: false,
            calibrated_keys: vec![],
            calibration_modal: None,
            calibration_v2: calibration_v2::State::default(),
            snap_tap: None,
            snap_observer: Default::default(),
            analog_observer: Default::default(),
            gamepad_observer: Default::default(),
            power_indicator: power::IndicatorState::default(),
            button_drawer_open: false,
            analog_gamepad: analog_gamepad::State::default(),
            analog_controls: analog_controls::State::default(),
            gaming_combined: gaming_combined::State::default(),
            dual_link_widget: None,
            source_artwork: source_artwork::ArtworkState::default(),
            keymap_bar: keymap_bar::State::default(),
            mapping_login_warning: mapping_login_warning::State::default(),
            mapping_capabilities:
                razer_model::mapping_requirements::MappingCapabilities::from_source_config(
                    &spec.config,
                )
                .map_err(|error| format!("{error:#}")),
            legacy_mapping: legacy_mapping::State::new(window),
            // Keep this gated by the independently traced mounted caller,
            // rather than the presence of an unused shared source component.
            properties_icon: properties::supported(pid).then(|| {
                if razer_platform::system::is_windows_11() {
                    "synapse/keyboard-properties-win11.svg"
                } else {
                    "synapse/keyboard-properties-legacy.svg"
                }
            }),
            brightness: brightness::State::default(),
            polling_connection: None,
            rgb_controls: None,
            lighting_observation: None,
            lighting_runtime: lighting_runtime::State::default(),
        };
        this.subscriptions.push(cx.observe_in(
            &this.ui_preferences,
            window,
            |this, _, window, cx| {
                this.resume_calibration_after_preference(window, cx);
                cx.notify();
            },
        ));
        if this.draft.pointer("/brightness/value").is_some() {
            this.add_slider("/brightness/value", 0., 100., 1., window, cx);
        }
        if this
            .draft
            .pointer("/switchOffLighting/idleMinutes")
            .is_some()
        {
            this.add_slider("/switchOffLighting/idleMinutes", 1., 15., 1., window, cx);
        }
        if spec.controls["dim_kind"] == "slider" {
            this.add_slider("/dimLighting/value", 1., 15., 1., window, cx);
        }
        if let Some(bounds) = spec.controls["power_bounds"].as_array() {
            let path = if spec.controls["power_kind"] == "mouse_slider" {
                "/powerSavingValue"
            } else {
                "/powerSaving/value"
            };
            this.add_slider(
                path,
                bounds[0].as_f64().unwrap() as f32,
                bounds[1].as_f64().unwrap() as f32,
                bounds[2].as_f64().unwrap() as f32,
                window,
                cx,
            );
        }
        this.init_actuation(window, cx);
        this.init_calibration_v2(cx);
        this.init_snap_tap(window, cx);
        this.init_rgb_controls(window, cx);
        this.init_keymap_bar(window, cx);
        this.restore_analog_controls();
        this.restore_gaming_combos();
        this.refresh_gaming_fn_sleep(cx);
        cx.on_release(|this, cx| {
            this.gamepad_observer.release(cx.background_executor());
            this.analog_observer.release(cx.background_executor());
            this.snap_observer.release(cx.background_executor());
        })
        .detach();
        this
    }
    pub fn set_page(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.calibration_v2_before_page(key, cx) {
            return;
        }
        if !self.mapping_before_page(key, cx) {
            return;
        }
        self.update_keymap_bar_page(key);
        self.source_history_page_bypass(key, window, cx);
    }

    pub fn dismiss_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_gaming_recording(false, window, cx);
        self.leave_analog_controls(window, cx);
        self.mapping_login_warning.unmount();
        self.clear_analog_mapping(cx);
        self.dismiss_calibration(window, cx);
    }
    /// The current 756 root's history handlers bypass changeView's calibration
    /// confirmation. The outer router selects this only for that source path.
    pub fn source_history_page_bypass(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != key {
            self.finish_gaming_recording(false, window, cx);
            self.leave_analog_controls(window, cx);
            self.mapping_login_warning.unmount();
            self.source_artwork.dismiss_special();
            if self.page == "TAB_LIGHTING" {
                self.cancel_lighting_requests();
            }
            if self.page == "TAB_CALIBRATION" {
                self.unmount_calibration_v2_page();
            }
            self.dismiss_rgb_controls(cx);
            self.leave_snap_tap(window, cx);
            self.leave_analog_observation(cx);
            self.stop_gamepad_observation(cx);
            self.dismiss_calibration(window, cx);
            self.clear_actuation_selection();
            self.clear_analog_mapping(cx);
            self.analog_gamepad.floating.unmount();
            self.page = key.into();
            if key == "TAB_CUSTOMIZE" {
                cx.emit(KeyboardMappingLoginWarningReadRequested);
            }
            if key == "TAB_LIGHTING" {
                self.refresh_lighting(cx);
            }
            if key == "TAB_CALIBRATION" {
                let flag = match self.spec.product_id {
                    740 | 746 => Some(UiPreference::KeyboardCalibrationIntro),
                    756 => Some(UiPreference::MagneticCalibrationSkipPrompt),
                    _ => None,
                };
                if let Some(flag) = flag {
                    self.ui_preferences
                        .update(cx, |owner, cx| owner.load(flag, cx));
                }
                self.enter_calibration_v2(window, cx);
            }
            if key == "TAB_POWER" {
                self.restart_indicator_animation();
            }
            self.sync_gamepad_observation(cx);
            cx.notify();
        }
    }

    pub fn observe_source_artwork(
        &mut self,
        observation: SourceArtworkObservation,
        cx: &mut Context<Self>,
    ) {
        if observation.product_id != self.spec.product_id {
            return;
        }
        if self.source_artwork.observation().is_none_or(|previous| {
            previous.product_id != observation.product_id
                || previous.edition_id != observation.edition_id
                || previous.layout_id != observation.layout_id
        }) {
            self.analog_gamepad.floating.key_bounds.borrow_mut().clear();
        }
        self.analog_gamepad.layout = Some(observation.layout_id);
        self.source_artwork.observe(observation);
        cx.notify();
    }
    fn keyboard_shapes(&self) -> &'static [razer_assets::KeyboardKey] {
        self.source_artwork
            .observation()
            .and_then(|observation| {
                source_artwork::geometry_for_surface(observation, self.page == "TAB_CUSTOMIZE")
            })
            .map(|geometry| geometry.keys)
            .unwrap_or(&self.spec.shapes)
    }

    pub fn set_factory_default_profile(
        &mut self,
        factory_default_profile: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.factory_default_profile == factory_default_profile {
            return;
        }
        self.cancel_pending_calibration_start();
        self.factory_default_profile = factory_default_profile;
        if factory_default_profile {
            self.finish_gaming_recording(false, window, cx);
            self.leave_analog_controls(window, cx);
            self.dismiss_calibration(window, cx);
            self.clear_actuation_selection();
            self.clear_analog_mapping(cx);
        }
        cx.notify();
    }
    pub fn snapshot(&self) -> Value {
        let mut snapshot = self.draft.clone();
        self.snap_snapshot(&mut snapshot);
        self.analog_mapping_snapshot(&mut snapshot);
        self.analog_controls_snapshot(&mut snapshot);
        self.latest_source_snapshot(&mut snapshot);
        self.gaming_combos_snapshot(&mut snapshot);
        snapshot
    }
    pub fn restore(&mut self, value: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pending_calibration_start();
        self.finish_gaming_recording(false, window, cx);
        self.leave_analog_controls(window, cx);
        self.cancel_lighting_requests();
        self.dismiss_rgb_controls(cx);
        self.cancel_actuation_connection();
        self.invalidate_brightness();
        self.dismiss_calibration(window, cx);
        self.draft = self.spec.default_profile();
        if let (Some(target), Some(saved)) =
            (self.draft.as_object_mut(), value.and_then(Value::as_object))
        {
            target.extend(
                saved
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        }
        self.selected_key = None;
        self.hovered_key = None;
        self.analog_gamepad.floating.cancel_pointer();
        self.restore_analog_mapping(window, cx);
        self.restore_analog_controls();
        self.restore_gaming_combos();
        self.restore_latest_source_gaming_mode();
        self.restore_linked_game_active(cx);
        self.refresh_gaming_fn_sleep(cx);
        self.init_keymap_bar(window, cx);
        self.restore_snap_tap(window, cx);
        self.clear_actuation_selection();
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if path == "/brightness/value" {
                continue;
            }
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        self.syncing = false;
        self.sync_rgb_controls(window, cx);
        self.request_brightness_read(cx);
        if self.page == "TAB_LIGHTING" {
            self.refresh_lighting(cx);
        }
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
        let value = self
            .draft
            .pointer(path)
            .and_then(Value::as_f64)
            .unwrap_or(min as f64) as f32;
        let slider = cx.new(|_| {
            let state = SliderState::new().min(min).max(max).step(step);
            if path == "/brightness/value" {
                state
            } else {
                state.default_value(value)
            }
        });
        let path_owned = path.to_owned();
        self.subscriptions.push(
            cx.subscribe_in(&slider, window, move |this, _, event, _, cx| {
                if !this.syncing {
                    if this.lighting_path_disabled(&path_owned) {
                        return;
                    }
                    if path_owned == "/brightness/value" {
                        if let SliderEvent::Change(value) = event {
                            this.preview_brightness(
                                value.start().clamp(min, max).round() as u8,
                                cx,
                            );
                        }
                        if let SliderEvent::Release(value) = event {
                            let percent = value.start().clamp(min, max).round() as u8;
                            this.draft["brightness"]["value"] = json!(percent);
                            this.draft["brightness"]["isEnabled"] = json!(percent != 0);
                            cx.emit(KeyboardProductChanged);
                            this.request_brightness(cx);
                            cx.notify();
                        }
                        // Current Range owns pointer preview; its default emits
                        // the parent's changeValue only on pointer release.
                        return;
                    }
                    if path_owned == "/switchOffLighting/idleMinutes" {
                        if let SliderEvent::Release(value) = event {
                            this.write(
                                &path_owned,
                                json!(value.start().clamp(min, max).round() as i64),
                                cx,
                            );
                        }
                        return;
                    }
                    if let SliderEvent::Change(value) = event {
                        this.write(
                            &path_owned,
                            json!(value.start().clamp(min, max).round() as i64),
                            cx,
                        );
                    }
                }
            }),
        );
        self.sliders.insert(path.into(), slider);
    }
    fn write(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        if self.page == "TAB_CUSTOMIZE"
            && self.customize_controls_disabled()
            && !path.starts_with("/brightness/")
            && !path.starts_with("/switchOffLighting/")
        {
            return;
        }
        if self.lighting_path_disabled(path) {
            return;
        }
        if self.draft.pointer(path) == Some(&value) {
            return;
        }
        if let Some((parent, key)) = path.rsplit_once('/') {
            if let Some(target) = self
                .draft
                .pointer_mut(parent)
                .and_then(Value::as_object_mut)
            {
                target.insert(key.into(), value);
                // Current756 UI reducer calls this dimKeyboardLighting while
                // ON_SET_DIM_LIGHTING persists payload.dimLighting at root.
                if self.spec.product_id == 756
                    && (path == "/dimKeyboardLighting" || path.starts_with("/dimKeyboardLighting/"))
                {
                    self.draft["dimLighting"] = self.draft["dimKeyboardLighting"].clone();
                }
                if path.starts_with("/switchOffLighting/") {
                    self.request_switch_off_lighting(cx);
                }
                if self.spec.modern_analog_controls()
                    && (path == "/gamingMode" || path.starts_with("/gamingMode/"))
                {
                    self.request_source_gaming_mode(cx);
                }
                cx.emit(KeyboardProductChanged);
                cx.notify();
            }
        }
    }
    fn toggle(&self, path: &str, label: String, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let path = path.to_owned();
        Checkbox::new(SharedString::from(format!("keyboard-{path}")))
            .label(label)
            .checked(
                self.draft
                    .pointer(&path)
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .disabled(!enabled)
            .on_click(cx.listener(move |this, value, window, cx| {
                if enabled {
                    this.write(&path, json!(value), cx);
                    if path == "/brightness/isEnabled" {
                        this.request_brightness(cx);
                        this.sync_brightness_controls(window, cx);
                    }
                }
            }))
            .into_any_element()
    }
    /// `kI`/`xI`: the switch-off-lighting widget. The audited source renders it
    /// as its own `.widget` — title `SWITCH_OFF_LIGHTING_HEADER`, tips
    /// `SWITCH_OFF_LIGHTING_TOOLTIP`, `extraClass:"has-slider"` — holding a
    /// `checkDisplay` check item (`DISPLAY_TURNED_OFF`) and, unless
    /// `DeviceInfo.hideLightingIdle` is set, a `checkIdle` check item
    /// (`IDLE_FOR_MIN`) plus a 1–15 minute slider with `minTag:"1"`/`maxTag:"15"`
    /// and no label. Both check items are disabled while brightness is off, and
    /// the slider is active only when the idle switch and brightness are on.
    fn switch_off_lighting(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let on = self
            .draft
            .pointer("/brightness/isEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let [(display_path, display_key), (idle_path, idle_key)] = SWITCH_OFF_LIGHTING_ITEMS;
        let display_on = self.boolean(display_path);
        let idle_on = self.boolean(idle_path);
        let mut widget = surface::panel_with_help(
            t("SWITCH_OFF_LIGHTING_HEADER"),
            surface::help_control(
                "keyboard-switch-off-lighting-help",
                t("SWITCH_OFF_LIGHTING_TOOLTIP"),
            ),
            cx,
        )
        .when(self.spec.analog_gamepad_layout(), |widget| widget.mb_0())
        .child(
            surface::check_item(
                "keyboard-switch-off-display",
                t(display_key),
                display_on,
                !on || self.lighting_controls_disabled(),
                window,
                cx,
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.write(display_path, json!(!display_on), cx)),
            ),
        );
        if !self.spec.config["DeviceInfo"]["hideLightingIdle"]
            .as_bool()
            .unwrap_or(false)
        {
            widget =
                widget
                    .child(
                        surface::check_item(
                            "keyboard-switch-off-idle",
                            t(idle_key),
                            idle_on,
                            !on || self.lighting_controls_disabled(),
                            window,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.write(idle_path, json!(!idle_on), cx)
                        })),
                    )
                    .child({
                        // `.has-slider .slider-container{margin-left:30px;width:490px}`
                        // 让滑条与勾选框文字对齐。
                        let mut column = v_flex().ml(surface::css(30.)).w(surface::css(490.));
                        if let Some(slider) = self.sliders.get("/switchOffLighting/idleMinutes") {
                            column =
                                column.child(Slider::new(slider).disabled(
                                    !(on && idle_on) || self.lighting_controls_disabled(),
                                ));
                        }
                        let (min, max) = SWITCH_OFF_IDLE_MINUTES;
                        column.child(surface::slider_tags(
                            &min.to_string(),
                            None,
                            &max.to_string(),
                            None,
                        ))
                    });
        }
        widget.into_any_element()
    }

    fn boolean(&self, path: &str) -> bool {
        self.draft
            .pointer(path)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// The `displayMode=chromaApp` popup mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn lighting_element(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.lighting(window, cx, true)
    }
    /// The lighting page (`ql`): `.body-widgets` with a left `.widget-col`
    /// holding the brightness widget and the switch-off-lighting widget, and a
    /// right column holding the effects widget.
    fn lighting(&self, window: &mut Window, cx: &mut Context<Self>, iframe: bool) -> AnyElement {
        let on = self
            .draft
            .pointer("/brightness/isEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let brightness = surface::panel_with_title_switch(
            t("BRIGHTNESS_HEADER"),
            surface::SynapseSwitch::new("keyboard-brightness-enabled")
                .accessibility_label(t("BRIGHTNESS_HEADER"))
                .checked(on)
                .disabled(self.lighting_controls_disabled())
                .on_change(cx.listener(|this, value: &bool, window, cx| {
                    this.write("/brightness/isEnabled", json!(*value), cx);
                    this.request_brightness(cx);
                    this.sync_brightness_controls(window, cx);
                })),
            surface::help_control("keyboard-brightness-help", t("BRIGHTNESS_TOOLTIP")),
            cx,
        )
        .when(self.spec.analog_gamepad_layout(), |widget| widget.mb_0())
        .children(self.sliders.get("/brightness/value").map(|slider| {
            v_flex()
                .child(self.brightness_slider(slider, !on || self.lighting_controls_disabled(), cx))
                .child(surface::slider_tags("0", None, "100", None))
        }));
        let mut left = v_flex().child(brightness);
        if self.draft.get("switchOffLighting").is_some() {
            left = left.child(self.switch_off_lighting(window, cx));
        }
        let columns = if self.spec.analog_gamepad_layout() {
            layout::columns()
                .gap_0()
                .child(layout::column(left.pb(surface::css(10.))))
                .child(layout::column(
                    v_flex()
                        .pb(surface::css(10.))
                        .child(self.huntsman_effects(cx)),
                ))
        } else if lighting_effects::radio_effects(self.spec.product_id) {
            layout::columns()
                .child(layout::column(left))
                .child(layout::column(self.radio_effects_panel(window, cx)))
        } else {
            layout::columns()
                .child(layout::column(left))
                .child(layout::column(self.huntsman_effects(cx)))
        };
        self.lighting_page_gates(
            columns
                .opacity(if self.lighting_columns_disabled() { 0.3 } else { 1. })
                .into_any_element(),
            iframe,
            window,
            cx,
        )
    }
    fn mapping_path(&self) -> String {
        if let Some(keymaps) = self.draft["keymaps"].as_array() {
            if let Some(index) = keymaps
                .iter()
                .position(|k| k["guid"] == self.draft["activeKeymapId"])
            {
                return format!("/keymaps/{index}/mappings");
            }
        }
        "/mappings".into()
    }
    fn gaming_mode(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.draft["gamingMode"]["state"].as_u64().unwrap_or(0);
        let enabled = state != 0;
        let windows_disabled = self.draft["gamingMode"]["isWindowsKeyDisabled"]
            .as_bool()
            .unwrap_or(false);
        let disabled = self.customize_controls_disabled();
        let rows = gaming_rows::for_product(self.spec.product_id);
        // Current 515 main.f60ca5aa.js NA.render/checkInGameOnly and
        // handleToggleGameMode read enableInGame independently of mode.state.
        // In particular, an observed IN_GAME mode must not rewrite the saved
        // checkbox preference or the next title-switch submission.
        let in_game = self.draft["gamingMode"]["enableInGame"]
            .as_bool()
            .unwrap_or(false);
        let mut gaming_help = t("GAMING_MODE_TOOLTIP");
        // Current 742/756 Gaming caller supplies both localized fragments
        // when this product declares supportGameModeGlobal.
        if self.spec.config["DeviceInfo"]["supportGameModeGlobal"].as_bool() == Some(true) {
            gaming_help.push_str(&t("GAMING_MODE_TOOLTIP_GLOBAL"));
        }
        let set_mode = |this: &mut Self, state: u64, in_game: bool, cx: &mut Context<Self>| {
            let mut value = this.draft["gamingMode"].clone();
            value["state"] = json!(state);
            value["enableInGame"] = json!(in_game);
            value["isWindowsKeyDisabled"] = json!(state != 0);
            this.write("/gamingMode", value, cx);
        };
        // Independently resolved in all 52 mounted current product components:
        // `hasSwitch:!0` belongs to the title; the body's first control is
        // `APPLY_IN_GAME_ONLY`, followed by a break and `GAMING_MODE_DESC`.
        surface::panel_with_title_switch(
            t("GAMING_MODE_HEADER"),
            surface::SynapseSwitch::new("keyboard-game-mode")
                .accessibility_label(t("GAMING_MODE_HEADER"))
                .checked(enabled)
                .disabled(disabled)
                .on_change(cx.listener(move |this, value: &bool, _, cx| {
                    set_mode(
                        this,
                        if *value {
                            if in_game { 2 } else { 1 }
                        } else {
                            0
                        },
                        in_game,
                        cx,
                    )
                })),
            surface::help_control("keyboard-gaming-mode-help", gaming_help),
            cx,
        )
        .relative()
        .when(self.spec.analog_customize_layout(), |widget| widget.mb_0())
        .when_some(self.spec.gaming_shortcut(), |panel, shortcut| panel.child(
            h_flex().absolute().top(surface::css(30.)).right(surface::css(35.)).items_baseline()
                .children(["FN",shortcut].into_iter().enumerate().map(|(index,label)| {
                    h_flex().items_baseline().when(index == 1, |key| key.child("+"))
                        .child(div().h(surface::css(27.)).px(surface::css(15.)).mx(surface::css(5.)).border_1().border_color(razer_widgets::theme::KeyboardAnalogControlColors::border()).rounded(surface::css(3.)).text_color(razer_widgets::theme::KeyboardAnalogControlColors::text()).flex().items_center().justify_center().child(label))
                }))))
        .child(
            Checkbox::new("keyboard-game-mode-in-game")
                .label(t("APPLY_IN_GAME_ONLY"))
                .checked(in_game)
                .disabled(!enabled || disabled)
                .on_click(cx.listener(move |this, value, _, cx| {
                    set_mode(this, if *value { 2 } else { 1 }, *value, cx)
                })),
        )
        .child(
            div()
                .mt(surface::css(17.))
                .mb(surface::css(10.))
                .when(!enabled, |description| description.opacity(0.3))
                .child(t("GAMING_MODE_DESC")),
        )
        .child(
            Checkbox::new("keyboard-game-mode-windows")
                .label(t("DISABLE_WINDOWS_KEY"))
                .checked(windows_disabled)
                .disabled(true),
        )
        .children(
            // Each allowlisted product's current parent chain and Menu
            // branch were checked independently; isSystem isn't assumed.
            (rows.is_some_and(|rows| rows.menu)
                && self
                    .spec
                    .keys
                    .iter()
                    .any(|key| key["inputID"] == "KEY_APPLICATION"))
            .then(|| {
                Checkbox::new("keyboard-game-mode-menu")
                    .label(t("DISABLE_MENU_KEY"))
                    .checked(windows_disabled)
                    .disabled(true)
            }),
        )
        .children(
            // The source keys and explicit read-only branch are checked
            // per product, including the separate 717 and 724 parents.
            (rows.is_some_and(|rows| rows.copilot)
                && self
                    .spec
                    .keys
                    .iter()
                    .any(|key| matches!(key["inputID"].as_str(), Some("DKM_D2" | "DKM_F6"))))
            .then(|| {
                Checkbox::new("keyboard-game-mode-copilot")
                    .label(t("DISABLE_COPILOT_KEY"))
                    .checked(windows_disabled)
                    .disabled(true)
            }),
        )
        .child(self.toggle(
            "/gamingMode/isAltTabDisabled",
            t("DISABLE_ALT_TAB"),
            enabled && !disabled,
            cx,
        ))
        .children(
            (!self.spec.config["DeviceInfo"]["notSupportAltF4"]
                .as_bool()
                .unwrap_or(false))
            .then(|| {
                self.toggle(
                    "/gamingMode/isAltF4Disabled",
                    t("DISABLE_ALT_F4"),
                    enabled && !disabled,
                    cx,
                )
            }),
        )
        .children(
            (self.spec.config["DeviceInfo"]["isShowDisabledAltSpace"].as_bool() == Some(true))
                .then(|| {
                    self.toggle(
                        "/gamingMode/isAltSpaceDisabled",
                        t("DISABLE_ALT_SPACE"),
                        enabled && !disabled,
                        cx,
                    )
                }),
        )
        .children(self.gaming_fn_sleep_control(enabled && !disabled,cx))
        .when(self.spec.custom_gaming_combinations(), |panel|panel
            .child(div().w_full().border_1().border_color(razer_widgets::theme::KeyboardAnalogControlColors::border()).mt(surface::css(14.)).mb(surface::css(24.)))
            .child(self.gaming_combined_keys(enabled,disabled,cx)))
        .into_any_element()
    }
    fn keyboard_image(&self, interactive: bool, cx: &Context<Self>) -> AnyElement {
        let keypad = matches!(self.spec.product_id, 555 | 580 | 747);
        let observation = self.source_artwork.observation();
        let factory = self.customize_factory_root();
        let frame = layout::KeyboardImageFrame::new(
            self.spec,
            observation,
            self.page == "TAB_CUSTOMIZE",
            factory,
        );
        let [width, height] = frame.size;
        let source_scale = frame.source_scale;
        let source_config_frame = frame.source_config_frame;
        let factory = source_config_frame && factory;
        let image = frame.image;
        let [image_width, image_height] = frame.image_size;
        let [image_left, image_top] = frame.image_origin;
        let [keyboard_left, keyboard_top] = frame.keyboard_origin;
        let keyboard_bounds = self.analog_gamepad.floating.keyboard_bounds.clone();
        let config_bounds = self.analog_gamepad.floating.config_bounds.clone();
        // All 72 current keyboard deployments retain the final 385px
        // .config-wrapper / 387px .config-block rules. Keypads own a separate
        // left column and must not acquire another shared 770px wrapper.
        let mut keyboard = div()
            .relative()
            .left(surface::css(keyboard_left))
            .top(surface::css(keyboard_top))
            .w(surface::css(width))
            .h(surface::css(height))
            .mx_auto()
            .on_prepaint(move |bounds, _, _| keyboard_bounds.set(bounds))
            .flex_shrink_0();
        if matches!(self.spec.product_id, 555 | 580 | 747) {
            keyboard = keyboard.child(self.keypad_product_image());
        } else if let Some(observation) =
            observation.filter(|_| source_artwork::has_source(self.spec.product_id))
        {
            keyboard = keyboard.child(SourceBaseArtwork::new(observation, &frame));
        } else if let Some(image) = image {
            keyboard = keyboard.child(
                img(SharedString::from(image.to_owned()))
                    .absolute()
                    .left(surface::css(image_left))
                    .top(surface::css(image_top))
                    .w(surface::css(image_width))
                    .h(surface::css(image_height))
                    .object_fit(ObjectFit::Contain),
            );
        }
        if let Some(wrist) = self.source_artwork.wrist() {
            let [x, y, w, h] = wrist.bounds;
            keyboard = keyboard.child(
                img(wrist.asset)
                    .absolute()
                    .left(surface::css(x))
                    .top(surface::css(y + wrist.transform_y))
                    .w(surface::css(w))
                    .h(surface::css(h))
                    .when(!wrist.visible, |image| image.invisible()),
            );
        }
        if interactive {
            keyboard = keyboard.children(self.keyboard_shapes().iter().map(|key| {
                let id = key.id.clone();
                let hover_id = id.clone();
                let selected = if self.page == "TAB_CALIBRATION" && self.spec.product_id == 756 {
                    self.calibration_v2.selected.contains(&id)
                } else if self.page == "ACTUATION" {
                    self.actuation
                        .as_ref()
                        .is_some_and(|state| state.selected.contains(&id))
                } else {
                    self.selected_key.as_ref() == Some(&id)
                };
                let hovered = self.hovered_key.as_ref() == Some(&id);
                let [x, y, w, h] = key.bounds;
                let drop_bounds = self.analog_gamepad.floating.key_bounds.clone();
                let drop_region = format!("{id}-{x}-{y}");
                // Current 515/678/679/688/742 SVG callers consume the
                // updateButtonWithMappings producer's `isMapped`, not merely
                // the presence of a profile mapping or actuation-only entry.
                let mapped = self.keyboard_physical_mapped(&id);
                div()
                    // 717 has two physical FN regions with one logical input.
                    // Scope the native child identity by its source geometry.
                    .id(SharedString::from(format!("keyboard-region-{id}-{x}-{y}")))
                    .absolute()
                    .left(surface::css(x * source_scale))
                    .top(surface::css(y * source_scale))
                    .w(surface::css(w * source_scale))
                    .h(surface::css(h * source_scale))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        if *hovered {
                            this.hover_source_special(&key.id, cx);
                        }
                    }))
                    .on_prepaint(move |bounds, _, _| {
                        drop_bounds
                            .borrow_mut()
                            .insert(drop_region.clone(), (key, bounds));
                    })
                    .child(
                        if self.page == "TAB_CALIBRATION" && self.spec.product_id == 756 {
                            self.calibration_v2_key(key, cx)
                        } else {
                            crate::keyboard_geometry::KeyRegion::new(
                                key,
                                if keypad && self.keypad_physical_fill_suppressed(&id) {
                                    cx.theme().transparent
                                } else if mapped {
                                    if self.spec.analog_customize_layout() || keypad {
                                        // Customize callers do not pass keyHoverAndRemappedColor;
                                        // the special black-remap class is therefore absent.
                                        if self.hypershift {
                                            Hsla::from(rgba(0xfd8611b3))
                                        } else if selected || hovered {
                                            cx.theme().transparent
                                        } else {
                                            Hsla::from(rgba(0x44d62c66))
                                        }
                                    } else {
                                        cx.theme().primary.opacity(0.4)
                                    }
                                } else {
                                    cx.theme().transparent
                                },
                                if (self.spec.analog_customize_layout() || keypad)
                                    && self.hypershift
                                {
                                    rgb(0xfd8611).into()
                                } else {
                                    cx.theme().primary
                                },
                                selected,
                                hovered,
                                cx.listener(move |this, event: &ClickEvent, window, cx| {
                                    if this.page == "TAB_CUSTOMIZE"
                                        && this.customize_controls_disabled()
                                    {
                                        return;
                                    }
                                    if this.page == "TAB_CALIBRATION" && this.spec.product_id == 756
                                    {
                                        this.select_calibration_v2_key(
                                            &id,
                                            event.modifiers().control,
                                            cx,
                                        );
                                    } else if this.page == "ACTUATION" {
                                        this.select_actuation_key(&id, window, cx);
                                    } else if this.page == "TAB_CUSTOMIZE" {
                                        if this.keypad_physical_click(
                                            &id,
                                            matches!(
                                                &key.geometry,
                                                razer_assets::KeyboardGeometry::Path { .. }
                                            ),
                                            cx,
                                        ) {
                                            return;
                                        }
                                        this.analog_mapping_next(
                                            analog_gamepad::MappingNext::Key(id.clone(), false),
                                            window,
                                            cx,
                                        );
                                    } else {
                                        this.selected_key = Some(id.clone());
                                    }
                                    cx.notify();
                                }),
                                cx.listener(move |this, hover, _, cx| {
                                    if *hover {
                                        this.hovered_key = Some(hover_id.clone());
                                    } else if this.hovered_key.as_ref() == Some(&hover_id) {
                                        this.hovered_key = None;
                                    }
                                    cx.notify();
                                }),
                            )
                            .tooltip_enabled(!keypad)
                            .disabled(
                                !key.enabled
                                    || (self.page == "TAB_CUSTOMIZE"
                                        && !self.keyboard_physical_enabled(&key.id))
                                    || (self.page == "TAB_CUSTOMIZE"
                                        && self.customize_controls_disabled())
                                    || (self.page == "TAB_CALIBRATION"
                                        && self.spec.product_id == 756
                                        && !self.calibration_v2_key_enabled(&key.id))
                                    || (self.page == "ACTUATION"
                                        && !self.actuation_key_enabled(&key.id)),
                            )
                            .into_any_element()
                        },
                    )
            }));
        }
        if self.page == "TAB_CALIBRATION" && matches!(self.spec.product_id, 740 | 746) {
            keyboard = self.legacy_calibration_keyboard(keyboard, source_scale, cx);
        }
        if self.page == "TAB_CALIBRATION" && self.spec.product_id == 756 {
            keyboard = self.calibration_v2_keyboard(keyboard, cx);
        }
        keyboard = keyboard.children(self.source_special_panels(cx));
        // PID580 main.1648cf28.js renders the keypad SVG inside the left
        // .config-block column. The shared 770px .config-wrapper belongs to
        // the entire keypad and its two right-hand panels, not this 390px
        // column; retaining another wrapper here shifts the SVG by 190px.
        if keypad && self.page == "TAB_CUSTOMIZE" {
            return keyboard.into_any_element();
        }
        surface::config_wrapper()
            .on_prepaint(move |bounds, _, _| config_bounds.set(bounds))
            // Current CSS across all keyboard products shares this frame;
            // product image height must not collapse the dot background.
            .when(source_config_frame, |wrapper| wrapper.h(surface::css(385.)))
            .child(surface::dot_background(cx))
            .child(if source_config_frame {
                let padding = if factory {
                    0.
                } else if matches!(self.spec.product_id, 719 | 720 | 741 | 742) {
                    25.
                } else {
                    self.spec.source_keyboard_top_padding_px
                };
                // CSS percentage padding is relative to the config-block width.
                h_flex()
                    .items_start()
                    .w_full()
                    .h(surface::css(387.))
                    .max_h(relative(1.))
                    .mt(surface::css(6.))
                    .justify_center()
                    .pt(relative(if factory {
                        0.
                    } else {
                        self.spec.source_keyboard_top_padding_percent / 100.
                    }))
                    .when(padding != 0., |block| block.pt(surface::css(padding)))
                    .when(
                        self.spec.product_id == 756 && self.observed_bluetooth() == Some(true),
                        |block| block.opacity(0.4),
                    )
                    .child(keyboard)
                    .into_any_element()
            } else {
                keyboard.into_any_element()
            })
            .children(
                (self.page == "TAB_CUSTOMIZE" && self.customize_factory_root())
                    .then(|| self.factory_customize_widget(cx)),
            )
            .into_any_element()
    }
    /// The `displayMode=armory` root mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn customize_element(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        self.customize(window, cx)
    }

    pub fn attach_dual_link_widget(
        &mut self,
        widget: Entity<super::dock_pairing::DualLinkWidget>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id == 756 {
            self.dual_link_widget = Some(widget);
            cx.notify();
        }
    }
    fn hypershift_row(&self, cx: &Context<Self>) -> AnyElement {
        // All 71 current product renders place this two-state switch after
        // `.config-wrapper`; labels resolve to STANDARD/HYPERSHIFT and the
        // orange active state is source CSS #fd8611.
        h_flex()
            .justify_center()
            .items_center()
            .mt(surface::css(20.))
            .mb(surface::css(10.))
            .gap(surface::css(10.))
            .when(self.spec.analog_customize_layout(), |row| {
                row.child(
                    gpui_kit::base::Button::new("keyboard-controller-toggle")
                        .accessibility_label(t("KEYBOARD_ANALOG_OPTIONS"))
                        .w(surface::css(38.))
                        .h(surface::css(27.))
                        .border_1()
                        .border_color(if self.analog_gamepad.floating.open {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x5d5d5d)
                        })
                        .rounded(surface::css(14.))
                        .bg(rgb(0x111111))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(if self.analog_gamepad.floating.open {
                                "synapse/keyboard-floating-controller-fill-green-icon.68e5616a.svg"
                            } else {
                                "synapse/keyboard-floating-controller-fill-grey-icon.a9ec3fff.svg"
                            })
                            .size(surface::css(18.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.analog_gamepad.floating.toggle();
                            cx.notify();
                        })),
                )
            })
            .child(
                gpui_kit::base::Button::new("keyboard-button-drawer-toggle")
                    .accessibility_label(t("TAB_CUSTOMIZE"))
                    .w(surface::css(38.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(if self.button_drawer_open {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x5d5d5d)
                    })
                    .rounded(surface::css(14.))
                    .bg(rgb(0x111111))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(if self.button_drawer_open {
                            "synapse/icon_sidepanel_a.svg"
                        } else {
                            "synapse/icon_sidepanel.svg"
                        })
                        .size(surface::css(18.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.analog_mapping_next(analog_gamepad::MappingNext::Drawer, window, cx);
                        cx.notify();
                    })),
            )
            .child(
                gpui_kit::base::Button::new("keyboard-hypershift")
                    .disabled(self.customize_controls_disabled())
                    .accessibility_label(t("HYPERSHIFT"))
                    .flex()
                    .items_center()
                    .h(surface::css(36.))
                    .p(surface::css(5.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .rounded(surface::css(18.))
                    .bg(rgb(0x111111))
                    .hover(|button| button.border_color(rgb(0x44d62c)))
                    .active(|button| button.bg(rgb(0x292929)))
                    .child(
                        div()
                            .h(surface::css(24.))
                            .px(surface::css(10.))
                            .pt(surface::css(6.))
                            .pb(surface::css(5.))
                            .mr(surface::css(5.))
                            .rounded(surface::css(12.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .bg(if self.hypershift {
                                rgba(0x00000000)
                            } else {
                                rgba(0x44d62cff)
                            })
                            .text_color(if self.hypershift {
                                rgb(0xcccccc)
                            } else {
                                rgb(0x212121)
                            })
                            .child(t("STANDARD")),
                    )
                    .child(
                        div()
                            .h(surface::css(24.))
                            .px(surface::css(10.))
                            .pt(surface::css(6.))
                            .pb(surface::css(5.))
                            .rounded(surface::css(12.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .bg(if self.hypershift {
                                rgba(0xfd8611ff)
                            } else {
                                rgba(0x00000000)
                            })
                            .text_color(if self.hypershift {
                                rgb(0x212121)
                            } else {
                                rgb(0xcccccc)
                            })
                            .child(t("HYPERSHIFT")),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.customize_controls_disabled() {
                            return;
                        }
                        this.analog_mapping_next(analog_gamepad::MappingNext::Hyper, window, cx);
                    })),
            )
            .child(layout::hypershift_help(self.spec.product_id))
            .into_any_element()
    }
    fn customize_contents(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let mut panel = v_flex();
        panel = panel.child(if matches!(self.spec.product_id, 555 | 580 | 747) {
            self.keypad_customize_artwork(cx)
        } else {
            self.keyboard_image(true, cx)
        });
        panel = panel.child(self.hypershift_row(cx));
        let mut page = v_flex().child(panel);
        // Modern products put their mounted widget in a route chunk, so the
        // main-bundle-only `controls` extraction misses it. Use actual callers.
        let gaming_mode = gaming_rows::for_product(self.spec.product_id).is_some();
        let snap_visible = !self.spec.analog_customize_layout()
            && snap_tap::snap_tap_visible(self.spec.product_id)
            && self.snap_tap.is_some();
        let snap_left = snap_visible && snap_tap::snap_tap_on_left(self.spec.product_id);
        let snap_right = snap_visible && snap_tap::snap_tap_on_right(self.spec.product_id);
        let snap_full = snap_visible && snap_tap::snap_tap_full_width(self.spec.product_id);
        let polling = self.polling_panel(cx);
        let polling_visible = polling.is_some();
        if self.spec.analog_customize_layout() {
            let columns = if self.spec.modern_analog_controls() {
                layout::columns()
                    .gap_0()
                    .child(layout::column(
                        v_flex()
                            .pb(surface::css(10.))
                            .when(self.spec.product_id == 756, |column| {
                                column.child(
                                    v_flex()
                                        .when(
                                            self.spec.product_id == 756
                                                && self.observed_bluetooth() == Some(true),
                                            |group| group.opacity(0.3),
                                        )
                                        .child(self.gaming_mode(cx))
                                        .child(self.analog_interrupt_panel(cx)),
                                )
                            })
                            .when(self.spec.product_id != 756, |column| {
                                column.child(self.gaming_mode(cx))
                            })
                            .children(self.keyboard_properties(cx))
                            .child(self.analog_gamepad_input_panel(cx))
                            .when(self.spec.product_id != 756, |column| {
                                column.child(self.analog_interrupt_panel(cx))
                            }),
                    ))
                    .child(layout::column(
                        v_flex()
                            .pb(surface::css(10.))
                            .child(
                                v_flex()
                                    .when(
                                        self.spec.product_id == 756
                                            && self.observed_bluetooth() == Some(true),
                                        |group| group.opacity(0.3),
                                    )
                                    .children(self.dual_link_widget.clone())
                                    .children(self.polling_panel(cx)),
                            )
                            .when(self.spec.source_mod_tap, |column| {
                                column.child(self.huntsman_mod_tap(cx))
                            })
                            .child(self.analog_deadzone_panel(cx))
                            .child(self.huntsman_quick_gamepad_settings(cx)),
                    ))
            } else {
                layout::columns()
                    .gap_0()
                    .child(layout::column(
                        v_flex()
                            .pb(surface::css(10.))
                            .child(self.gaming_mode(cx))
                            .children(self.keyboard_properties(cx)),
                    ))
                    .child(layout::column(
                        v_flex()
                            .pb(surface::css(10.))
                            .when(self.spec.source_mod_tap, |column| {
                                column.child(self.huntsman_mod_tap(cx))
                            })
                            .child(self.huntsman_quick_remapping(cx))
                            .child(self.huntsman_gamepad_tester(cx)),
                    ))
            };
            let page = page.child(columns).into_any_element();
            return div()
                .relative()
                .w_full()
                .child(
                    h_flex()
                        .items_start()
                        .children(
                            self.button_drawer_open
                                .then(|| self.analog_mapping_drawer(cx)),
                        )
                        .child(div().flex_1().min_w_0().child(page)),
                )
                .children(self.analog_mapping_popup(window, cx).map(|popup| {
                    div()
                        .absolute()
                        .top(surface::css(90.))
                        .left(surface::css(if self.button_drawer_open {
                            230.
                        } else {
                            20.
                        }))
                        .child(popup)
                }))
                .children(self.analog_mapping_confirmation(cx))
                .child(self.floating_controller_layer(cx))
                .into_any_element();
        }
        // Current Customize roots put BodyWidgets directly after the
        // `config-row mt20 mb10`; each widget already owns its 10px margin.
        if gaming_mode || snap_visible || self.properties_icon.is_some() || polling_visible {
            page = page.child(
                layout::columns()
                    .when(
                        gaming_mode
                            || snap_left
                            || self.properties_on_left()
                            || (polling_visible && self.polling_on_left()),
                        |columns| {
                            columns.child(layout::column(
                                v_flex()
                                    .when(gaming_mode, |column| column.child(self.gaming_mode(cx)))
                                    .when(snap_left, |column| column.children(self.snap_panel(cx)))
                                    .when(self.properties_on_left(), |column| {
                                        column.children(self.keyboard_properties(cx))
                                    })
                                    .when(self.polling_on_left(), |column| {
                                        column.children(self.polling_panel(cx))
                                    }),
                            ))
                        },
                    )
                    .when(
                        snap_right
                            || (!self.properties_on_left() && !self.properties_full_width())
                            || (polling_visible && !self.polling_on_left()),
                        |columns| {
                            columns.child(layout::column(
                                v_flex()
                                    .when(!self.polling_on_left(), |column| {
                                        column.children(self.polling_panel(cx))
                                    })
                                    .when(snap_right, |column| column.children(self.snap_panel(cx)))
                                    .when(
                                        !self.properties_on_left() && !self.properties_full_width(),
                                        |column| column.children(self.keyboard_properties(cx)),
                                    ),
                            ))
                        },
                    ),
            );
        }
        if snap_full {
            page = page.children(self.snap_panel(cx));
        }
        if self.properties_full_width() {
            page = page.children(self.keyboard_properties(cx));
        }
        self.mapping_customize_layer(page.into_any_element(), window, cx)
    }
}
impl Render for KeyboardProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_keypad_artwork_runtime(cx);
        self.sync_keypad_product_image(window, cx);
        let content = match self.page.as_str() {
            // Every current mounted Lighting root uses the settings columns;
            // none mounts Customize's product image above them. Exact roots,
            // hashes and UTF-16 ranges: keyboard-lighting-roots-2026-10-11.json.
            "TAB_LIGHTING" => self.lighting(window, cx, false),
            "TAB_CUSTOMIZE" => self.customize(window, cx),
            "ACTUATION" => self.actuation_page(window, cx),
            "TAB_CALIBRATION" if self.spec.product_id == 756 => {
                self.calibration_v2_page(window, cx)
            }
            "TAB_CALIBRATION" => self.calibration_page(window, cx),
            "TAB_POWER" => self.power(cx),
            _ => surface::panel(t(&self.page), cx)
                .child(self.spec.name.clone())
                .into_any_element(),
        };
        div()
            .id("keyboard-product-body")
            .test_support()
            .w_full()
            .flex_shrink_0()
            .text_color(cx.theme().foreground)
            .when(
                self.spec.source_mod_tap && self.page == "TAB_CUSTOMIZE",
                |body| {
                    body.capture_key_down(cx.listener(|this, _: &KeyDownEvent, _, cx| {
                        this.analog_gamepad.key_pressed = true;
                        cx.notify();
                    }))
                    .capture_key_up(cx.listener(
                        |this, _: &KeyUpEvent, _, cx| {
                            this.analog_gamepad.key_pressed = false;
                            cx.notify();
                        },
                    ))
                },
            )
            .child(super::product_surface::body().child(content))
            .children(self.calibration_modal.clone())
            .children(self.snap_overlay(window, cx))
    }
}

pub use analog_gamepad::{
    KeyboardMappingBrightnessObservation, KeyboardMappingBrightnessReadRequested,
};

// Generated catalog reader: razer-pages/keyboard_products_data.
fn read_keyboard_products_catalog() -> Vec<(u32, fn() -> KeyboardProductSpec)> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyboardProductSpec> {
        let value = match r.tag()? {
            0 => KeyboardProductSpec {
                product_id: r.child(decode_1)?,
                source_layout: None,
                source_mod_tap: Default::default(),
                source_keyboard_top_padding_percent: Default::default(),
                source_keyboard_top_padding_px: Default::default(),
                name: r.child(decode_2)?,
                config: r.child(decode_3)?,
                pages: r.child(decode_4)?,
                keys: r.child(decode_5)?,
                shapes: r.child(decode_6)?,
                viewbox: r.child(decode_18)?,
                image: r.child(decode_20)?,
                image_size: r.child(decode_18)?,
                controls: r.child(decode_3)?,
            },
            1 => KeyboardProductSpec {
                product_id: r.child(decode_1)?,
                source_layout: r.child(decode_20)?,
                source_mod_tap: Default::default(),
                source_keyboard_top_padding_percent: r.child(decode_19)?,
                source_keyboard_top_padding_px: r.child(decode_19)?,
                name: r.child(decode_2)?,
                config: r.child(decode_3)?,
                pages: r.child(decode_4)?,
                keys: r.child(decode_5)?,
                shapes: r.child(decode_6)?,
                viewbox: r.child(decode_18)?,
                image: r.child(decode_20)?,
                image_size: r.child(decode_18)?,
                controls: r.child(decode_3)?,
            },
            2 => KeyboardProductSpec {
                product_id: r.child(decode_1)?,
                source_layout: r.child(decode_20)?,
                source_mod_tap: r.child(decode_21)?,
                source_keyboard_top_padding_percent: r.child(decode_19)?,
                source_keyboard_top_padding_px: r.child(decode_19)?,
                name: r.child(decode_2)?,
                config: r.child(decode_3)?,
                pages: r.child(decode_4)?,
                keys: r.child(decode_5)?,
                shapes: r.child(decode_6)?,
                viewbox: r.child(decode_18)?,
                image: r.child(decode_20)?,
                image_size: r.child(decode_18)?,
                controls: r.child(decode_3)?,
            },
            3 => KeyboardProductSpec {
                product_id: r.child(decode_1)?,
                source_layout: None,
                source_mod_tap: r.child(decode_21)?,
                source_keyboard_top_padding_percent: Default::default(),
                source_keyboard_top_padding_px: Default::default(),
                name: r.child(decode_2)?,
                config: r.child(decode_3)?,
                pages: r.child(decode_4)?,
                keys: r.child(decode_5)?,
                shapes: r.child(decode_6)?,
                viewbox: r.child(decode_18)?,
                image: r.child(decode_20)?,
                image_size: r.child(decode_18)?,
                controls: r.child(decode_3)?,
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
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardKey>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardKey> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardKey {
                id: r.child(decode_8)?,
                label: r.child(decode_8)?,
                enabled: r.child(decode_9)?,
                functions: r.child(decode_10)?,
                bounds: r.child(decode_11)?,
                geometry: r.child(decode_13)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 4]> {
        let value = match r.tag()? {
            0 => r.array::<_, 4>(decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardGeometry> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardGeometry::Path {
                commands: r.child(decode_14)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardPathCommand>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_15)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardPathCommand> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardPathCommand::Move(r.child(decode_16)?),
            1 => razer_assets::KeyboardPathCommand::Line(r.child(decode_16)?),
            2 => razer_assets::KeyboardPathCommand::Curve(r.child(decode_17)?),
            3 => razer_assets::KeyboardPathCommand::Close,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[[f32; 2]; 3]> {
        let value = match r.tag()? {
            0 => r.array::<_, 3>(decode_16)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_19)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_2)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn product_515() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            515u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_521() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            521u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_529() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            529u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_534() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            534u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_542() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            542u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_545() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            545u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_550() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            550u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_551() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            551u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_552() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            552u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_554() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            554u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_555() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            555u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_556() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            556u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_565() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            565u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_567() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            567u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_574() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            574u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_575() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            575u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_577() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            577u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_579() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            579u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_580() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            580u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_585() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            585u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_590() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            590u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_591() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            591u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_592() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            592u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_593() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            593u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_599() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            599u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_600() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            600u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_602() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            602u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_605() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            605u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_606() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            606u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_614() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            614u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_617() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            617u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_619() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            619u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_620() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            620u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_631() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            631u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_642() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            642u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_647() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            647u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_654() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            654u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_655() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            655u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_658() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            658u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_659() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            659u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_660() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            660u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_661() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            661u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_664() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            664u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_673() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            673u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_674() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            674u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_675() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            675u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_677() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            677u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_678() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            678u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_679() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            679u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_688() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            688u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_689() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            689u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_691() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            691u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_697() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            697u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_708() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            708u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_716() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            716u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_717() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            717u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_719() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            719u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_720() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            720u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_721() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            721u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_724() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            724u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_727() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            727u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_728() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            728u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_739() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            739u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_740() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            740u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_741() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            741u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_742() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            742u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_746() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            746u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_747() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            747u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_752() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            752u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_756() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            756u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2595() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            2595u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2596() -> KeyboardProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PRODUCTS_DATA,
            2596u32,
            "af74d74ada0ba2b06efc23a930a8047001a2abdcbcd1bea5517e3d96f62dbc8b",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    vec![
        (515u32, product_515 as fn() -> KeyboardProductSpec),
        (521u32, product_521 as fn() -> KeyboardProductSpec),
        (529u32, product_529 as fn() -> KeyboardProductSpec),
        (534u32, product_534 as fn() -> KeyboardProductSpec),
        (542u32, product_542 as fn() -> KeyboardProductSpec),
        (545u32, product_545 as fn() -> KeyboardProductSpec),
        (550u32, product_550 as fn() -> KeyboardProductSpec),
        (551u32, product_551 as fn() -> KeyboardProductSpec),
        (552u32, product_552 as fn() -> KeyboardProductSpec),
        (554u32, product_554 as fn() -> KeyboardProductSpec),
        (555u32, product_555 as fn() -> KeyboardProductSpec),
        (556u32, product_556 as fn() -> KeyboardProductSpec),
        (565u32, product_565 as fn() -> KeyboardProductSpec),
        (567u32, product_567 as fn() -> KeyboardProductSpec),
        (574u32, product_574 as fn() -> KeyboardProductSpec),
        (575u32, product_575 as fn() -> KeyboardProductSpec),
        (577u32, product_577 as fn() -> KeyboardProductSpec),
        (579u32, product_579 as fn() -> KeyboardProductSpec),
        (580u32, product_580 as fn() -> KeyboardProductSpec),
        (585u32, product_585 as fn() -> KeyboardProductSpec),
        (590u32, product_590 as fn() -> KeyboardProductSpec),
        (591u32, product_591 as fn() -> KeyboardProductSpec),
        (592u32, product_592 as fn() -> KeyboardProductSpec),
        (593u32, product_593 as fn() -> KeyboardProductSpec),
        (599u32, product_599 as fn() -> KeyboardProductSpec),
        (600u32, product_600 as fn() -> KeyboardProductSpec),
        (602u32, product_602 as fn() -> KeyboardProductSpec),
        (605u32, product_605 as fn() -> KeyboardProductSpec),
        (606u32, product_606 as fn() -> KeyboardProductSpec),
        (614u32, product_614 as fn() -> KeyboardProductSpec),
        (617u32, product_617 as fn() -> KeyboardProductSpec),
        (619u32, product_619 as fn() -> KeyboardProductSpec),
        (620u32, product_620 as fn() -> KeyboardProductSpec),
        (631u32, product_631 as fn() -> KeyboardProductSpec),
        (642u32, product_642 as fn() -> KeyboardProductSpec),
        (647u32, product_647 as fn() -> KeyboardProductSpec),
        (654u32, product_654 as fn() -> KeyboardProductSpec),
        (655u32, product_655 as fn() -> KeyboardProductSpec),
        (658u32, product_658 as fn() -> KeyboardProductSpec),
        (659u32, product_659 as fn() -> KeyboardProductSpec),
        (660u32, product_660 as fn() -> KeyboardProductSpec),
        (661u32, product_661 as fn() -> KeyboardProductSpec),
        (664u32, product_664 as fn() -> KeyboardProductSpec),
        (673u32, product_673 as fn() -> KeyboardProductSpec),
        (674u32, product_674 as fn() -> KeyboardProductSpec),
        (675u32, product_675 as fn() -> KeyboardProductSpec),
        (677u32, product_677 as fn() -> KeyboardProductSpec),
        (678u32, product_678 as fn() -> KeyboardProductSpec),
        (679u32, product_679 as fn() -> KeyboardProductSpec),
        (688u32, product_688 as fn() -> KeyboardProductSpec),
        (689u32, product_689 as fn() -> KeyboardProductSpec),
        (691u32, product_691 as fn() -> KeyboardProductSpec),
        (697u32, product_697 as fn() -> KeyboardProductSpec),
        (708u32, product_708 as fn() -> KeyboardProductSpec),
        (716u32, product_716 as fn() -> KeyboardProductSpec),
        (717u32, product_717 as fn() -> KeyboardProductSpec),
        (719u32, product_719 as fn() -> KeyboardProductSpec),
        (720u32, product_720 as fn() -> KeyboardProductSpec),
        (721u32, product_721 as fn() -> KeyboardProductSpec),
        (724u32, product_724 as fn() -> KeyboardProductSpec),
        (727u32, product_727 as fn() -> KeyboardProductSpec),
        (728u32, product_728 as fn() -> KeyboardProductSpec),
        (739u32, product_739 as fn() -> KeyboardProductSpec),
        (740u32, product_740 as fn() -> KeyboardProductSpec),
        (741u32, product_741 as fn() -> KeyboardProductSpec),
        (742u32, product_742 as fn() -> KeyboardProductSpec),
        (746u32, product_746 as fn() -> KeyboardProductSpec),
        (747u32, product_747 as fn() -> KeyboardProductSpec),
        (752u32, product_752 as fn() -> KeyboardProductSpec),
        (756u32, product_756 as fn() -> KeyboardProductSpec),
        (2595u32, product_2595 as fn() -> KeyboardProductSpec),
        (2596u32, product_2596 as fn() -> KeyboardProductSpec),
    ]
}

#[cfg(test)]
mod keyboard_products_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_products_data_decodes() {
        for (_, factory) in super::read_keyboard_products_catalog() {
            let _ = factory();
        }
    }
}
// End generated catalog reader: razer-pages/keyboard_products_data.
