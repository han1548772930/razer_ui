//! Native drafts for current keyboard and keypad product sources.
//! Specifications are statically extracted; no vendor JavaScript is executed.
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
use razer_widgets::surface;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[path = "keyboard_actuation.rs"]
mod actuation;
#[path = "keyboard_actuation_submission.rs"]
mod actuation_submission;
pub use actuation_submission::KeyboardActuationRequested;
#[path = "keyboard_brightness.rs"]
mod brightness;
#[path = "keyboard_calibration.rs"]
mod calibration;
#[path = "keyboard_gaming_rows.rs"]
mod gaming_rows;
#[path = "keyboard_polling.rs"]
mod polling;
#[path = "keyboard_power.rs"]
mod power;
pub use polling::KeyboardPollingConnection;
pub use power::{KeyboardIndicatorLedObservation, KeyboardIndicatorLedRequested};
#[path = "keyboard_huntsman679.rs"]
mod huntsman679;
#[path = "keyboard_properties.rs"]
mod properties;
pub use brightness::KeyboardBrightnessReadRequested;
pub use brightness::KeyboardBrightnessRequested;
pub use huntsman679::KeyboardGamepadTesterObservation;
#[path = "keyboard_snap_tap.rs"]
mod snap_tap;
pub(super) use calibration::is_factory_profile;
pub use calibration::open_preview as open_calibration_preview;
pub use snap_tap::SnapTapObservation;

#[derive(Deserialize)]
pub struct KeyboardProductSpec {
    product_id: u32,
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
        if self.product_id == 691 {
            if let Some(profile) = profile.as_object_mut() {
                profile
                    .entry("oledLowBatteryWarningDisplay")
                    .or_insert_with(|| json!({"enabled":true,"value":20}));
            }
        }
        profile
    }
}
pub fn source_product(pid: u32) -> Option<&'static KeyboardProductSpec> {
    static PRODUCTS: OnceLock<Vec<KeyboardProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_products_data.json"))
                .expect("validated current keyboard specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}
pub struct KeyboardProductChanged;
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
    scroll: ScrollHandle,
    actuation: Option<actuation::State>,
    actuation_submission: actuation_submission::State,
    calibration_intro_visible: bool,
    /// Source `profileReducer.isFactoryDefaultProfile`; the calibration page
    /// renders a warning and disables its customize surface for this profile.
    factory_default_profile: bool,
    /// Explicit development workspace only; live calibration never fabricates events.
    calibration_preview: bool,
    calibration_modal: Option<Entity<calibration::CalibrationModal>>,
    snap_tap: Option<snap_tap::State>,
    /// Retained OS icon choice; failed queries fall back to the legacy icon.
    properties_icon: Option<&'static str>,
    brightness: brightness::State,
    polling_connection: Option<KeyboardPollingConnection>,
    power_indicator: power::IndicatorState,
    /// Current 679's source drawer is independent of its key mapping popup.
    button_drawer_open: bool,
    huntsman679: huntsman679::State,
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
            scroll: ScrollHandle::new(),
            actuation: None,
            actuation_submission: actuation_submission::State::default(),
            calibration_intro_visible: calibration::load_intro_visibility(),
            factory_default_profile,
            calibration_preview: false,
            calibration_modal: None,
            snap_tap: None,
            power_indicator: power::IndicatorState::default(),
            button_drawer_open: false,
            huntsman679: huntsman679::State::default(),
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
        };
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
        this.init_snap_tap(window, cx);
        this
    }
    pub fn set_page(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.leave_snap_tap(window, cx);
            self.dismiss_calibration(window, cx);
            self.clear_actuation_selection();
            self.page = key.into();
            if key == "TAB_POWER" {
                self.restart_indicator_animation();
            }
            self.scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
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
        self.factory_default_profile = factory_default_profile;
        if factory_default_profile {
            self.dismiss_calibration(window, cx);
            self.clear_actuation_selection();
        }
        cx.notify();
    }
    pub fn snapshot(&self) -> Value {
        let mut snapshot = self.draft.clone();
        self.snap_snapshot(&mut snapshot);
        snapshot
    }
    pub fn restore(&mut self, value: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
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
        self.restore_snap_tap(window, cx);
        self.clear_actuation_selection();
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        self.syncing = false;
        self.request_brightness_read(cx);
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
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(value)
        });
        let path_owned = path.to_owned();
        self.subscriptions.push(
            cx.subscribe_in(&slider, window, move |this, _, event, _, cx| {
                if !this.syncing {
                    if path_owned == "/brightness/value" {
                        if let SliderEvent::Change(value) = event {
                            this.preview_brightness(
                                Some(value.start().clamp(min, max).round() as u8),
                                cx,
                            );
                        }
                        if let SliderEvent::Release(value) = event {
                            this.preview_brightness(None, cx);
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
            .on_click(cx.listener(move |this, value, _, cx| {
                if enabled {
                    this.write(&path, json!(value), cx);
                    if path == "/brightness/isEnabled" {
                        this.request_brightness(cx);
                    }
                }
            }))
            .into_any_element()
    }
    fn range(&self, path: &str, label: String, enabled: bool) -> AnyElement {
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        let value = if path == "/brightness/value" {
            self.brightness_preview().map(|value| value.to_string())
        } else {
            None
        }
        .unwrap_or_else(|| {
            self.draft
                .pointer(path)
                .map(Value::to_string)
                .unwrap_or_default()
        });
        v_flex()
            .gap_3()
            .child(h_flex().justify_between().child(label).child(value))
            .child(Slider::new(slider).disabled(!enabled))
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
        let mut widget = surface::panel_with_control(
            t("SWITCH_OFF_LIGHTING_HEADER"),
            surface::help_control(
                "keyboard-switch-off-lighting-help",
                t("SWITCH_OFF_LIGHTING_TOOLTIP"),
            ),
            cx,
        )
        .child(
            surface::check_item(
                "keyboard-switch-off-display",
                t(display_key),
                display_on,
                !on,
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
                            !on,
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
                            column = column.child(Slider::new(slider).disabled(!(on && idle_on)));
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
        self.lighting(window, cx)
    }
    /// The lighting page (`ql`): `.body-widgets` with a left `.widget-col`
    /// holding the brightness widget and the switch-off-lighting widget, and a
    /// right column holding the effects widget.
    fn lighting(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
                .on_change(cx.listener(|this, value: &bool, _, cx| {
                    this.write("/brightness/isEnabled", json!(*value), cx);
                    this.request_brightness(cx);
                })),
            surface::help_control("keyboard-brightness-help", t("BRIGHTNESS_TOOLTIP")),
            cx,
        )
        .children(self.sliders.get("/brightness/value").map(|slider| {
            v_flex()
                .child(Slider::new(slider).disabled(!on))
                .child(surface::slider_tags("0", None, "100", None))
        }));
        let mut left = v_flex().child(brightness);
        if self.draft.get("switchOffLighting").is_some() {
            left = left.child(self.switch_off_lighting(window, cx));
        }
        if self.spec.product_id == 679 {
            return surface::page_columns()
                .child(surface::page_column(left))
                .child(surface::page_column(self.huntsman_effects(cx)))
                .into_any_element();
        }
        let mut right = surface::panel(t("QUICK_EFFECTS"), cx);
        if let Some(effects) = self.spec.config["QUICK_EFFECTS"].as_array() {
            right = right.children(effects.iter().filter_map(|effect| {
                let id = effect["id"].as_u64()?;
                let label = t(effect["name"].as_str()?);
                Some(
                    Button::new(SharedString::from(format!("keyboard-effect-{id}")))
                        .label(label)
                        .outline()
                        .selected(
                            self.draft["quickEffects"]["selectedEffectId"].as_u64() == Some(id),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.write("/quickEffects/selectedEffectId", json!(id), cx)
                        })),
                )
            }));
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
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
    fn assign_key(&mut self, input: &str, mut assignment: Value, cx: &mut Context<Self>) {
        let Some(key) = self
            .spec
            .keys
            .iter()
            .find(|key| key["inputID"].as_str() == Some(input))
        else {
            return;
        };
        if !key["isEnabled"].as_bool().unwrap_or(true) {
            return;
        }
        if key["inputType"] == "AnalogInput" && self.actuation.is_some() {
            self.assign_analog_key(input, assignment, cx);
            return;
        }
        assignment["inputID"] = json!(input);
        assignment["inputType"] = key["inputType"].clone();
        assignment["isHyperShift"] = json!(self.hypershift);
        match key["inputType"].as_str() {
            Some("KeyInput") => {
                assignment["keyInput"] = json!({"HID":key["HID"], "pageID":key["pageID"]})
            }
            Some("AnalogInput") => assignment["analogInput"] = json!({"Id":key["analogInputID"]}),
            _ => {}
        }
        let path = self.mapping_path();
        if let Some(mappings) = self.draft.pointer_mut(&path).and_then(Value::as_array_mut) {
            mappings.retain(|mapping| {
                !(mapping["inputID"].as_str() == Some(input)
                    && mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift)
            });
            mappings.push(assignment);
            cx.emit(KeyboardProductChanged);
            cx.notify();
        }
    }
    fn keymap_assignment(&self, input: &str) -> Option<Value> {
        self.draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)
            .and_then(|mappings| {
                mappings.iter().find(|mapping| {
                    mapping["inputID"].as_str() == Some(input)
                        && mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
                        && mapping["outputType"].as_str() == Some("keymapGroup")
                })
            })
            .cloned()
    }
    fn assign_keymap(
        &mut self,
        input: &str,
        kind: &str,
        keymap: Option<&Value>,
        clutch: bool,
        cx: &mut Context<Self>,
    ) {
        let mut group = json!({"type": kind, "isClutch": false});
        if kind == "specificKeymap" {
            let Some(keymap) = keymap else {
                return;
            };
            let Some(guid) = keymap["guid"].as_str() else {
                return;
            };
            group["guid"] = json!(guid);
            group["name"] = keymap["name"].clone();
            group["isClutch"] = json!(clutch);
            if clutch {
                if let Some(active) = self.draft["activeKeymapId"].as_str() {
                    group["previousKeymapGuid"] = json!(active);
                }
            }
        }
        self.assign_key(
            input,
            json!({
                "outputType": "keymapGroup",
                "keymapGroup": group,
            }),
            cx,
        );
    }
    fn assign_keyboard_mouse(
        &mut self,
        input: &str,
        assignment: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let path = self.mapping_path();
        let Some(mappings) = self.draft.pointer_mut(&path).and_then(Value::as_array_mut) else {
            return;
        };
        let Some(mapping) = mappings.iter_mut().find(|mapping| {
            mapping["inputID"].as_str() == Some(input)
                && mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
                && mapping["outputType"].as_str() == Some("keyboardGroup")
        }) else {
            return;
        };
        let Some(group) = mapping["keyboardGroup"].as_object_mut() else {
            return;
        };
        match assignment {
            Some(assignment) => {
                group.insert("mouseGroup".into(), json!({"mouseAssignment": assignment}));
            }
            None => {
                group.remove("mouseGroup");
            }
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn gaming_mode(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.draft["gamingMode"]["state"].as_u64().unwrap_or(0);
        let enabled = state != 0;
        let windows_disabled = self.draft["gamingMode"]["isWindowsKeyDisabled"]
            .as_bool()
            .unwrap_or(false);
        let rows = gaming_rows::for_product(self.spec.product_id);
        let in_game = state == 2
            || self.draft["gamingMode"]["enableInGame"]
                .as_bool()
                .unwrap_or(false);
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
            surface::help_control("keyboard-gaming-mode-help", t("GAMING_MODE_TOOLTIP")),
            cx,
        )
        .child(
            Checkbox::new("keyboard-game-mode-in-game")
                .label(t("APPLY_IN_GAME_ONLY"))
                .checked(in_game)
                .disabled(!enabled)
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
            enabled,
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
                    enabled,
                    cx,
                )
            }),
        )
        .into_any_element()
    }
    fn keyboard_image(&self, interactive: bool, cx: &Context<Self>) -> AnyElement {
        let [width, height] = self.spec.viewbox;
        let [image_width, image_height] = self.spec.image_size;
        let mut keyboard = div()
            .relative()
            .w(surface::css(width))
            .h(surface::css(height))
            .mx_auto()
            .flex_shrink_0();
        if let Some(image) = &self.spec.image {
            keyboard = keyboard.child(
                img(SharedString::from(image.clone()))
                    .absolute()
                    .left(surface::css((width - image_width) / 2.))
                    .top_0()
                    .w(surface::css(image_width))
                    .h(surface::css(image_height)),
            );
        }
        if interactive {
            keyboard = keyboard.children(self.spec.shapes.iter().map(|key| {
                let id = key.id.clone();
                let hover_id = id.clone();
                let selected = if self.page == "ACTUATION" {
                    self.actuation
                        .as_ref()
                        .is_some_and(|state| state.selected.contains(&id))
                } else {
                    self.selected_key.as_ref() == Some(&id)
                };
                let hovered = self.hovered_key.as_ref() == Some(&id);
                let [x, y, w, h] = key.bounds;
                let mapped = self
                    .draft
                    .pointer(&self.mapping_path())
                    .and_then(Value::as_array)
                    .is_some_and(|mappings| {
                        mappings.iter().any(|m| {
                            m["inputID"].as_str() == Some(&id)
                                && m["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
                        })
                    });
                div()
                    // 717 has two physical FN regions with one logical input.
                    // Scope the native child identity by its source geometry.
                    .id(SharedString::from(format!("keyboard-region-{id}-{x}-{y}")))
                    .absolute()
                    .left(surface::css(x))
                    .top(surface::css(y))
                    .w(surface::css(w))
                    .h(surface::css(h))
                    .child(
                        crate::keyboard_geometry::KeyRegion::new(
                            key,
                            if mapped && self.spec.product_id != 679 {
                                cx.theme().primary.opacity(0.4)
                            } else {
                                cx.theme().transparent
                            },
                            if self.spec.product_id == 679 && self.hypershift {
                                rgb(0xfd8611).into()
                            } else {
                                cx.theme().primary
                            },
                            selected,
                            hovered,
                            cx.listener(move |this, _, window, cx| {
                                if this.page == "ACTUATION" {
                                    this.select_actuation_key(&id, window, cx);
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
                        .disabled(
                            !key.enabled
                                || (self.page == "ACTUATION"
                                    && !self.actuation_key_enabled(&key.id)),
                        ),
                    )
            }));
        }
        surface::config_wrapper()
            // 679's final CSS overrides the earlier shared 340px rule.
            .when(
                self.spec.product_id == 679 && self.page == "TAB_CUSTOMIZE",
                |wrapper| wrapper.h(surface::css(385.)),
            )
            .child(surface::dot_background(cx))
            .child(keyboard.when(
                self.spec.product_id == 679 && self.page == "TAB_CUSTOMIZE",
                |keyboard| keyboard.mt(surface::css(6.)),
            ))
            .into_any_element()
    }
    /// The `displayMode=armory` root mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.customize(cx)
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
            .when(self.spec.product_id == 679, |row| {
                row.child(
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
                                "synapse/keyboard-679-sidepanel-active.svg"
                            } else {
                                "synapse/keyboard-679-sidepanel.svg"
                            })
                            .size(surface::css(18.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.button_drawer_open = !this.button_drawer_open;
                            cx.notify();
                        })),
                )
            })
            .child(
                gpui_kit::base::Button::new("keyboard-hypershift")
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.hypershift = !this.hypershift;
                        this.selected_key = None;
                        this.hovered_key = None;
                        cx.notify();
                    })),
            )
            .child(surface::help_control(
                "keyboard-hypershift-help",
                t("HYPERSHIFT_TOOLTIP"),
            ))
            .into_any_element()
    }
    fn customize(&self, cx: &Context<Self>) -> AnyElement {
        let mut panel = v_flex();
        if let Some(keymaps) = self.draft["keymaps"].as_array() {
            panel = panel.child(
                h_flex()
                    .gap_2()
                    .children(keymaps.iter().filter_map(|keymap| {
                        let id = keymap["guid"].as_str()?.to_owned();
                        let label = keymap["name"].as_str()?.to_owned();
                        Some(
                            Button::new(SharedString::from(format!("keyboard-keymap-{id}")))
                                .label(label)
                                .outline()
                                .selected(
                                    self.draft["activeKeymapId"].as_str() == Some(id.as_str()),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_key = None;
                                    this.write("/activeKeymapId", json!(id), cx);
                                })),
                        )
                    })),
            );
        }
        panel = panel.child(self.keyboard_image(true, cx));
        panel = panel.child(self.hypershift_row(cx));
        if self.spec.product_id == 679 && self.button_drawer_open {
            panel = panel.child(
                v_flex()
                    .w(surface::css(250.))
                    .max_h(surface::css(340.))
                    .id("keyboard-679-button-drawer")
                    .overflow_y_scroll()
                    .children(self.spec.keys.iter().filter_map(|key| {
                        let id = key["inputID"].as_str()?.to_owned();
                        let name = key["counter"]
                            .as_str()
                            .or_else(|| key["defaultValue"].as_str())
                            .unwrap_or(&id)
                            .to_owned();
                        Some(
                            Button::new(SharedString::from(format!("keyboard-679-drawer-{id}")))
                                .label(name)
                                .outline()
                                .disabled(!key["isEnabled"].as_bool().unwrap_or(true))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_key = Some(id.clone());
                                    cx.notify();
                                })),
                        )
                    })),
            );
        }
        panel = panel.child(
            h_flex().gap_2().flex_wrap().children(
                self.spec
                    .keys
                    .iter()
                    .filter(|key| {
                        !self
                            .spec
                            .shapes
                            .iter()
                            .any(|shape| key["inputID"].as_str() == Some(shape.id.as_str()))
                    })
                    .filter_map(|key| {
                        let id = key["inputID"].as_str()?.to_owned();
                        let label = key["counter"]
                            .as_str()
                            .or_else(|| key["defaultValue"].as_str())
                            .unwrap_or(&id)
                            .to_owned();
                        Some(
                            Button::new(SharedString::from(format!("keyboard-key-{id}")))
                                .label(label)
                                .outline()
                                .disabled(!key["isEnabled"].as_bool().unwrap_or(true))
                                .selected(self.selected_key.as_ref() == Some(&id))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_key = Some(id.clone());
                                    cx.notify();
                                })),
                        )
                    }),
            ),
        );
        // Source 679 opens the mounted side mapping blade (Ph/Pi) from OM.
        // The inline generic editor below belongs to older products and must
        // never be reachable for Huntsman V3 Pro TKL.
        if self.spec.product_id != 679 {
            if let Some(input) = self.selected_key.clone() {
                if let Some(key) = self
                    .spec
                    .keys
                    .iter()
                    .find(|k| k["inputID"].as_str() == Some(&input))
                {
                    let supported = |name: &str| {
                        key["functionList"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(name)))
                    };
                    panel = panel.child(div().font_bold().child(input.clone()));
                    if supported("DEFAULT") {
                        let input = input.clone();
                        panel = panel.child(
                            Button::new("keyboard-reset-mapping")
                                .label(t("DEFAULT"))
                                .outline()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.actuation.is_some() {
                                        this.reset_analog_assignment(&input, cx);
                                        return;
                                    }
                                    let path = this.mapping_path();
                                    let shift = this.hypershift;
                                    if let Some(mappings) =
                                        this.draft.pointer_mut(&path).and_then(Value::as_array_mut)
                                    {
                                        mappings.retain(|m| {
                                            !(m["inputID"].as_str() == Some(&input)
                                                && m["isHyperShift"].as_bool().unwrap_or(false)
                                                    == shift)
                                        });
                                        cx.emit(KeyboardProductChanged);
                                        cx.notify();
                                    }
                                })),
                        );
                    }
                    if supported("KEYBOARD_FUNCTION") {
                        panel=panel.child(div().child(t("KEYBOARD_FUNCTION"))).child(h_flex().gap_2().flex_wrap().children(self.spec.keys.iter().filter_map(|target| {
                        let hid=target["HID"].as_str()?.to_owned();let page=target["pageID"].as_str()?.to_owned();
                        let name=target["counter"].as_str()?.to_owned();let input=input.clone();
                        Some(Button::new(SharedString::from(format!("keyboard-target-{}",target["inputID"].as_str()?))).label(name).outline().on_click(cx.listener(move |this,_,_,cx| {
                            this.assign_key(&input,json!({"outputType":"keyboardGroup","keyboardGroup":{"key":{"HID":hid,"pageID":page,"flag":0},"modifiers":[]}}),cx);
                        })))
                    })));
                    }
                    if supported("MOUSE_FUNCTION") {
                        let keyboard_mapping = self
                            .draft
                            .pointer(&self.mapping_path())
                            .and_then(Value::as_array)
                            .and_then(|mappings| {
                                mappings.iter().find(|mapping| {
                                    mapping["inputID"].as_str() == Some(&input)
                                        && mapping["isHyperShift"].as_bool().unwrap_or(false)
                                            == self.hypershift
                                        && mapping["outputType"].as_str() == Some("keyboardGroup")
                                })
                            })
                            .cloned()
                            .unwrap_or_default();
                        let keyboard_group = keyboard_mapping["keyboardGroup"].clone();
                        let mouse_assignment = keyboard_group
                            .pointer("/mouseGroup/mouseAssignment")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                        let has_keyboard = keyboard_group.is_object();
                        let mouse_actions = [
                            ("Click", "TEXT_MOUSE_BIND_LEFT_CLICK"),
                            ("Menu", "TEXT_MOUSE_BIND_RIGHT_CLICK"),
                            ("ScrollButton", "TEXT_MOUSE_BIND_SCROLL_CLICK"),
                            ("Previous", "TEXT_MOUSE_BUTTON_4"),
                            ("Next", "TEXT_MOUSE_BUTTON_5"),
                            ("ScrollUp", "TEXT_MOUSE_BIND_SCROLL_UP"),
                            ("ScrollDown", "TEXT_MOUSE_BIND_SCROLL_DOWN"),
                        ];
                        panel = panel
                            .child(div().child(t("MOUSE_FUNCTION")))
                            .child(
                                Checkbox::new("keyboard-combine-mouse")
                                    .label(t("COMBINE_WITH_MOUSE"))
                                    .checked(mouse_assignment.is_some())
                                    .disabled(!has_keyboard)
                                    .on_change(cx.listener({
                                        let input = input.clone();
                                        move |this, value: &bool, _, cx| {
                                            this.assign_keyboard_mouse(
                                                &input,
                                                (*value).then_some("Click"),
                                                cx,
                                            );
                                        }
                                    })),
                            )
                            .child(h_flex().gap_2().flex_wrap().children(
                                mouse_actions.into_iter().map(|(assignment, label)| {
                                    let input = input.clone();
                                    let selected = mouse_assignment.as_deref() == Some(assignment);
                                    Button::new(SharedString::from(format!(
                                        "keyboard-mouse-target-{assignment}"
                                    )))
                                    .label(t(label))
                                    .outline()
                                    .selected(selected)
                                    .disabled(!has_keyboard || mouse_assignment.is_none())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.assign_keyboard_mouse(&input, Some(assignment), cx);
                                    }))
                                }),
                            ));
                    }
                    if supported("SWITCH_KEYMAP") {
                        let keymaps = self.draft["keymaps"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default();
                        let active_keymap =
                            self.draft["activeKeymapId"].as_str().map(str::to_owned);
                        let mut ordered_keymaps = keymaps.iter().collect::<Vec<_>>();
                        ordered_keymaps.sort_by_key(|keymap| {
                            keymap["slot"]
                                .as_str()
                                .and_then(|slot| slot.parse::<u32>().ok())
                                .unwrap_or(u32::MAX)
                        });
                        let first_keymap = ordered_keymaps
                            .first()
                            .and_then(|keymap| keymap["guid"].as_str())
                            .map(str::to_owned);
                        let last_keymap = ordered_keymaps
                            .last()
                            .and_then(|keymap| keymap["guid"].as_str())
                            .map(str::to_owned);
                        let has_multiple = keymaps.iter().any(|keymap| {
                            keymap["guid"].as_str().is_some()
                                && keymap["guid"].as_str() != active_keymap.as_deref()
                        });
                        let current = self.keymap_assignment(&input).unwrap_or_default();
                        let current_group = current["keymapGroup"].clone();
                        let current_type = current_group["type"].as_str().unwrap_or_default();
                        let mut actions = h_flex().gap_2().flex_wrap();
                        for (kind, label) in [
                            ("nextKeymap", "NEXT_KEYMAP"),
                            ("previousKeymap", "PREVIOUS_KEYMAP"),
                            ("cycleUpKeymap", "CYCLE_UP_KEYMAP"),
                            ("cycleDownKeymap", "CYCLE_DOWN_KEYMAP"),
                        ] {
                            let input = input.clone();
                            actions = actions.child(
                                Button::new(SharedString::from(format!(
                                    "keyboard-keymap-action-{kind}"
                                )))
                                .label(t(label))
                                .outline()
                                .selected(current_type == kind)
                                .disabled(match kind {
                                    "nextKeymap" => {
                                        !has_multiple
                                            || active_keymap.as_deref() == last_keymap.as_deref()
                                    }
                                    "previousKeymap" => {
                                        !has_multiple
                                            || active_keymap.as_deref() == first_keymap.as_deref()
                                    }
                                    _ => !has_multiple,
                                })
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.assign_keymap(&input, kind, None, false, cx);
                                    },
                                )),
                            );
                        }
                        panel = panel.child(div().child(t("SWITCH_KEYMAP"))).child(actions);
                        if has_multiple {
                            let specific_selected = current_type == "specificKeymap";
                            panel = panel.child(
                                Button::new("keyboard-keymap-specific")
                                    .label(t("SPECIFIC_KEYMAP"))
                                    .outline()
                                    .selected(specific_selected)
                                    .on_click(cx.listener({
                                        let input = input.clone();
                                        let keymaps = keymaps.clone();
                                        let active_keymap = active_keymap.clone();
                                        move |this, _, _, cx| {
                                            if let Some(keymap) = keymaps.iter().find(|keymap| {
                                                keymap["guid"].as_str().is_some()
                                                    && keymap["guid"].as_str()
                                                        != active_keymap.as_deref()
                                            }) {
                                                this.assign_keymap(
                                                    &input,
                                                    "specificKeymap",
                                                    Some(keymap),
                                                    false,
                                                    cx,
                                                );
                                            }
                                        }
                                    })),
                            );
                            let mut specific = h_flex().gap_2().flex_wrap();
                            for keymap in keymaps.iter().filter(|keymap| {
                                keymap["guid"].as_str().is_some()
                                    && keymap["guid"].as_str() != active_keymap.as_deref()
                            }) {
                                let Some(guid) = keymap["guid"].as_str() else {
                                    continue;
                                };
                                let name = keymap["name"].as_str().unwrap_or("Keymap").to_owned();
                                let selected = current_group["guid"].as_str() == Some(guid);
                                let keymap = keymap.clone();
                                let input = input.clone();
                                specific = specific.child(
                                    Button::new(SharedString::from(format!(
                                        "keyboard-keymap-specific-{guid}"
                                    )))
                                    .label(name)
                                    .outline()
                                    .selected(selected)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.assign_keymap(
                                            &input,
                                            "specificKeymap",
                                            Some(&keymap),
                                            false,
                                            cx,
                                        );
                                    })),
                                );
                            }
                            panel = panel.child(specific);
                            if specific_selected {
                                if let Some(keymap) = keymaps.iter().find(|keymap| {
                                    keymap["guid"].as_str() == current_group["guid"].as_str()
                                }) {
                                    let input = input.clone();
                                    let keymap = keymap.clone();
                                    panel = panel.child(
                                        Checkbox::new("keyboard-keymap-clutch")
                                            .label(t("SWITCH_BACK_TO_PREVIOUS_KEYMAP_DESCRIPTION"))
                                            .checked(
                                                current_group["isClutch"]
                                                    .as_bool()
                                                    .unwrap_or(false),
                                            )
                                            .on_click(cx.listener(move |this, value, _, cx| {
                                                this.assign_keymap(
                                                    &input,
                                                    "specificKeymap",
                                                    Some(&keymap),
                                                    *value,
                                                    cx,
                                                );
                                            })),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut page = v_flex().gap_5().child(panel);
        // Modern products put their mounted widget in a route chunk, so the
        // main-bundle-only `controls` extraction misses it. Use actual callers.
        let gaming_mode = gaming_rows::for_product(self.spec.product_id).is_some();
        let snap_visible = self.spec.product_id != 679
            && snap_tap::snap_tap_visible(self.spec.product_id)
            && self.snap_tap.is_some();
        let snap_left = snap_visible && snap_tap::snap_tap_on_left(self.spec.product_id);
        let snap_right = snap_visible && snap_tap::snap_tap_on_right(self.spec.product_id);
        let snap_full = snap_visible && snap_tap::snap_tap_full_width(self.spec.product_id);
        let polling = self.polling_panel(cx);
        let polling_visible = polling.is_some();
        if self.spec.product_id == 679 {
            return page
                .child(
                    surface::page_columns()
                        .child(surface::page_column(
                            v_flex()
                                .child(self.gaming_mode(cx))
                                .children(self.keyboard_properties(cx)),
                        ))
                        .child(surface::page_column(
                            v_flex()
                                .child(self.huntsman_quick_remapping(cx))
                                .child(self.huntsman_gamepad_tester(cx)),
                        )),
                )
                .into_any_element();
        }
        if gaming_mode || snap_visible || self.properties_icon.is_some() || polling_visible {
            page = page.child(
                surface::page_columns()
                    .when(
                        gaming_mode
                            || snap_left
                            || self.properties_on_left()
                            || (polling_visible && self.polling_on_left()),
                        |columns| {
                            columns.child(surface::page_column(
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
                            columns.child(surface::page_column(
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
        page.into_any_element()
    }
}
impl Render for KeyboardProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page.as_str() {
            "TAB_LIGHTING" => {
                if self.spec.product_id == 679 {
                    // $L -> em mounts only left KD/xD and right VL; no product image.
                    self.lighting(window, cx)
                } else {
                    v_flex()
                        .gap_5()
                        .child(self.keyboard_image(false, cx))
                        .child(self.lighting(window, cx))
                        .into_any_element()
                }
            }
            "TAB_CUSTOMIZE" => self.customize(cx),
            "ACTUATION" => self.actuation_page(cx),
            "TAB_CALIBRATION" => self.calibration_page(window, cx),
            "TAB_POWER" => self.power(cx),
            _ => surface::panel(t(&self.page), cx)
                .child(self.spec.name.clone())
                .into_any_element(),
        };
        div()
            .id("keyboard-product-body")
            .test_support()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_color(cx.theme().foreground)
            .child(super::product_surface::body().child(content))
            .children(self.calibration_modal.clone())
            .children(self.snap_overlay(window, cx))
    }
}
