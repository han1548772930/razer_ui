//! Native drafts for current keyboard and keypad product sources.
//! Specifications are statically extracted; no vendor JavaScript is executed.
use crate::{i18n::t, ui::surface};
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, StyledExt,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[path = "keyboard_actuation.rs"]
mod actuation;
#[path = "keyboard_calibration.rs"]
mod calibration;
pub(super) use calibration::is_factory_profile;
pub(crate) use calibration::open_preview as open_calibration_preview;

#[derive(Deserialize)]
pub(crate) struct KeyboardProductSpec {
    product_id: u32,
    name: String,
    config: Value,
    pages: Vec<String>,
    keys: Vec<Value>,
    shapes: Vec<crate::resources::KeyboardKey>,
    viewbox: [f32; 2],
    image: Option<String>,
    image_size: [f32; 2],
    controls: Value,
}
impl KeyboardProductSpec {
    pub(crate) fn macro_keys(&self) -> &[Value] {
        &self.keys
    }
    pub(crate) fn macro_shapes(&self) -> &[crate::resources::KeyboardKey] {
        &self.shapes
    }
    pub(crate) fn macro_image(&self) -> Option<&str> {
        self.image.as_deref()
    }
    pub(crate) fn macro_viewbox(&self) -> [f32; 2] {
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
pub(crate) fn source_product(pid: u32) -> Option<&'static KeyboardProductSpec> {
    static PRODUCTS: OnceLock<Vec<KeyboardProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_products_data.json"))
                .expect("validated current keyboard specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}
pub(crate) struct KeyboardProductChanged;
/// `kI` 的两个勾选项：状态路径与文案键（源码别名 `Rt.ISS`/`Rt.CUC` 解析为
/// `DISPLAY_TURNED_OFF`、`IDLE_FOR_MIN`）。
pub(crate) const SWITCH_OFF_LIGHTING_ITEMS: [(&str, &str); 2] = [
    ("/switchOffLighting/isDisplayOn", "DISPLAY_TURNED_OFF"),
    ("/switchOffLighting/isIdleEnabled", "IDLE_FOR_MIN"),
];

/// 空闲滑条的量程：`min:1`、`max:15`、`step:1`、`minTag:"1"`、`maxTag:"15"`。
pub(crate) const SWITCH_OFF_IDLE_MINUTES: (i64, i64) = (1, 15);

#[cfg(test)]
#[path = "keyboard_products_tests.rs"]
mod tests;

pub(crate) struct KeyboardProductWorkspace {
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
    calibration_intro_visible: bool,
    /// Source `profileReducer.isFactoryDefaultProfile`; the calibration page
    /// renders a warning and disables its customize surface for this profile.
    factory_default_profile: bool,
    /// Explicit development workspace only; live calibration never fabricates events.
    calibration_preview: bool,
    calibration_modal: Option<Entity<calibration::CalibrationModal>>,
}
impl EventEmitter<KeyboardProductChanged> for KeyboardProductWorkspace {}
impl KeyboardProductWorkspace {
    pub(crate) fn new(
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
            calibration_intro_visible: calibration::load_intro_visibility(),
            factory_default_profile,
            calibration_preview: false,
            calibration_modal: None,
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
        this
    }
    pub(crate) fn set_page(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.dismiss_calibration(window, cx);
            if let Some(state) = &mut self.actuation {
                state.selected.clear();
            }
            self.page = key.into();
            self.scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }

    pub(crate) fn set_factory_default_profile(
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
        }
        cx.notify();
    }
    pub(crate) fn snapshot(&self) -> Value {
        self.draft.clone()
    }
    pub(crate) fn restore(
        &mut self,
        value: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if let Some(state) = &mut self.actuation {
            state.selected.clear();
        }
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                slider.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        self.syncing = false;
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
                }
            }))
            .into_any_element()
    }
    fn range(&self, path: &str, label: String, enabled: bool) -> AnyElement {
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        v_flex()
            .gap_3()
            .child(
                h_flex().justify_between().child(label).child(
                    self.draft
                        .pointer(path)
                        .map(Value::to_string)
                        .unwrap_or_default(),
                ),
            )
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
    pub(crate) fn lighting_element(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        let mut left = surface::panel(t("BRIGHTNESS"), cx)
            .child(self.toggle("/brightness/isEnabled", t("BRIGHTNESS"), true, cx))
            .child(self.range("/brightness/value", t("BRIGHTNESS"), on));
        if self.draft.get("switchOffLighting").is_some() {
            left = left.child(self.switch_off_lighting(window, cx));
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
    fn choices(
        &self,
        path: &str,
        options: &[Value],
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .children(options.iter().filter_map(|value| {
                let value = value.as_u64()?;
                let path = path.to_owned();
                Some(
                    Button::new(SharedString::from(format!(
                        "keyboard-choice-{path}-{value}"
                    )))
                    .label(crate::i18n::t_value("MIN", value as i64))
                    .outline()
                    .selected(self.draft.pointer(&path).and_then(Value::as_u64) == Some(value))
                    .disabled(!enabled)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.write(&path, json!(value), cx)),
                    ),
                )
            }))
            .into_any_element()
    }
    fn power(&self, cx: &Context<Self>) -> AnyElement {
        if self.spec.product_id == 691 {
            return self.power_691(cx);
        }
        let mut left = v_flex().gap_5();
        let mut right = v_flex().gap_5();
        if let Some(kind) = self.spec.controls["dim_kind"].as_str() {
            let path = if kind == "choices" {
                "/dimKeyboardLighting"
            } else {
                "/dimLighting"
            };
            let enabled = self
                .draft
                .pointer(&format!("{path}/isEnabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let panel = surface::panel(t("DIM_KEYBOARD_LIGHTING_TITLE"), cx)
                .child(surface::note(t("DIM_KEYBOARD_LIGHTING_DESC"), cx))
                .child(self.toggle(&format!("{path}/isEnabled"), t("IDLE_FOR_MIN"), true, cx));
            left = left.child(if kind == "choices" {
                panel.child(
                    self.choices(
                        &format!("{path}/value"),
                        self.spec.config["DIM_KEYBOARD_LIGHTING_VALUES"]
                            .as_array()
                            .unwrap(),
                        enabled,
                        cx,
                    ),
                )
            } else {
                panel.child(self.range(&format!("{path}/value"), String::new(), enabled))
            });
        }
        if let Some(kind) = self.spec.controls["power_kind"].as_str() {
            let enabled = kind == "mouse_slider"
                || self.draft["powerSaving"]["isEnabled"]
                    .as_bool()
                    .unwrap_or(false);
            let mut panel = surface::panel(t("KEYBOARD_POWER_SAVING_TITLE"), cx)
                .child(surface::note(t("KEYBOARD_POWER_SAVING_DESC"), cx));
            if kind != "mouse_slider" {
                panel =
                    panel.child(self.toggle("/powerSaving/isEnabled", t("IDLE_FOR_MIN"), true, cx));
            }
            right = right.child(if kind == "choices" {
                panel.child(
                    self.choices(
                        "/powerSaving/value",
                        self.spec.config["KEYBOARD_WIRELESS_POWER_SAVING_VALUES"]
                            .as_array()
                            .unwrap(),
                        enabled,
                        cx,
                    ),
                )
            } else {
                panel.child(self.range(
                    if kind == "mouse_slider" {
                        "/powerSavingValue"
                    } else {
                        "/powerSaving/value"
                    },
                    String::new(),
                    enabled,
                ))
            });
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
    /// Current 691 Power `y/u`: switches are part of the title and choices
    /// contain raw numeric labels. The other three source widgets are pending.
    fn power_691(&self, cx: &Context<Self>) -> AnyElement {
        let widget = |path: &'static str,
                      title: &'static str,
                      description: &'static str,
                      help: &'static str,
                      values: &'static str| {
            let enabled_path = format!("{path}/isEnabled");
            let value_path = format!("{path}/value");
            let enabled = self
                .draft
                .pointer(&enabled_path)
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let switch =
                surface::SynapseSwitch::new(SharedString::from(format!("691-power-{path}")))
                    .accessibility_label(t(title))
                    .checked(enabled)
                    .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                        this.write(&enabled_path, json!(*enabled), cx)
                    }));
            let choices = h_flex()
                .relative()
                .flex_wrap()
                .gap(surface::css(10.))
                .children(
                    self.spec.config[values]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|value| {
                            let value = value.as_u64()?;
                            let value_path = value_path.clone();
                            let selected = self.draft.pointer(&value_path).and_then(Value::as_u64)
                                == Some(value);
                            Some(
                                gpui_kit::base::Button::new(SharedString::from(format!(
                                    "691-power-{path}-{value}"
                                )))
                                .accessibility_label(value.to_string())
                                .disabled(!enabled)
                                .w(surface::css(48.))
                                .h(surface::css(27.))
                                .rounded(surface::css(3.))
                                .bg(rgb(0x222222))
                                .text_size(surface::css(14.))
                                .text_color(rgb(0xcccccc))
                                .border_1()
                                .border_color(if selected {
                                    rgb(0x44d62c)
                                } else {
                                    rgb(0x5d5d5d)
                                })
                                .hover(|button| button.border_color(rgb(0x44d62c)))
                                .child(value.to_string())
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.write(&value_path, json!(value), cx),
                                )),
                            )
                        }),
                )
                .when(!enabled, |row| {
                    row.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .w(surface::css(300.))
                            .h(surface::css(30.))
                            .bg(rgb(0x111111))
                            .opacity(0.5)
                            .occlude(),
                    )
                });
            surface::panel_with_title_switch(t(title), switch, div(), cx)
                .relative()
                .child(
                    div()
                        .absolute()
                        .top(surface::css(10.))
                        .right(surface::css(10.))
                        .child(surface::help_control(
                            SharedString::from(format!("691-power-help-{path}")),
                            t(help),
                        )),
                )
                .child(div().child(t(description)))
                .child(div().mt(surface::css(20.)).child(choices))
        };
        surface::page_columns()
            .child(surface::page_column(widget(
                "/dimKeyboardLighting",
                "DIM_LIGHTING_HEADER",
                "DIM_KEYBOARD_LIGHTING_DESC",
                "DIM_KEYBOARD_LIGHTING_TIPS",
                "DIM_KEYBOARD_LIGHTING_VALUES",
            )))
            .child(surface::page_column(widget(
                "/powerSaving",
                "KEYBOARD_POWER_SAVING_TITLE",
                "KEYBOARD_POWER_SAVING_DESC",
                "KEYBOARD_POWER_SAVING_TIPS",
                "KEYBOARD_WIRELESS_POWER_SAVING_VALUES",
            )))
            .into_any_element()
    }
    fn gaming_mode(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.draft["gamingMode"]["state"].as_u64().unwrap_or(0);
        let enabled = state != 0;
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
        surface::panel(t("GAMING_MODE_HEADER"), cx)
            .child(
                Checkbox::new("keyboard-game-mode")
                    .label(t("GAME_MODE"))
                    .checked(enabled)
                    .on_click(cx.listener(move |this, value, _, cx| {
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
            )
            .child(
                Checkbox::new("keyboard-game-mode-in-game")
                    .label(t("GAMING_MODE_IN_GAME"))
                    .checked(in_game)
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, value, _, cx| {
                        set_mode(this, if *value { 2 } else { 1 }, *value, cx)
                    })),
            )
            .child(
                Checkbox::new("keyboard-game-mode-windows")
                    .label(t("DISABLE_WINDOWS_KEY"))
                    .checked(enabled)
                    .disabled(true),
            )
            .children(
                // Current 716 Gaming Mode: T is derived from the button list;
                // the read-only Copilot row mirrors isWindowsKeyDisabled.
                // Other products require their own mounted-source audit.
                (self.spec.product_id == 716
                    && self
                        .spec
                        .keys
                        .iter()
                        .any(|key| matches!(key["inputID"].as_str(), Some("DKM_D2" | "DKM_F6"))))
                .then(|| {
                    Checkbox::new("keyboard-game-mode-copilot")
                        .label(t("DISABLE_COPILOT_KEY"))
                        .checked(
                            self.draft["gamingMode"]["isWindowsKeyDisabled"]
                                .as_bool()
                                .unwrap_or(false),
                        )
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
                        crate::ui::keyboard_geometry::KeyRegion::new(
                            key,
                            if mapped {
                                cx.theme().primary.opacity(0.4)
                            } else {
                                cx.theme().transparent
                            },
                            cx.theme().primary,
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
            .child(surface::dot_background(cx))
            .child(keyboard)
            .into_any_element()
    }
    /// The `displayMode=armory` root mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub(crate) fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.customize(cx)
    }
    fn customize(&self, cx: &Context<Self>) -> AnyElement {
        let mut panel = surface::panel(t("TAB_CUSTOMIZE"), cx);
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
        panel = panel.child(
            Checkbox::new("keyboard-hypershift")
                .label("Razer Hypershift")
                .checked(self.hypershift)
                .on_click(cx.listener(|this, value, _, cx| {
                    this.hypershift = *value;
                    cx.notify();
                })),
        );
        panel = panel.child(self.keyboard_image(true, cx));
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
                    panel =
                        panel
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
                    let active_keymap = self.draft["activeKeymapId"].as_str().map(str::to_owned);
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
                            specific =
                                specific.child(
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
                                            current_group["isClutch"].as_bool().unwrap_or(false),
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
        let mut page = v_flex().gap_5().child(panel);
        if self.spec.controls["gaming_mode"].as_bool().unwrap_or(false) {
            page = page
                .child(surface::page_columns().child(surface::page_column(self.gaming_mode(cx))));
        }
        page.into_any_element()
    }
}
impl Render for KeyboardProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page.as_str() {
            "TAB_LIGHTING" => v_flex()
                .gap_5()
                .child(self.keyboard_image(false, cx))
                .child(self.lighting(window, cx))
                .into_any_element(),
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
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_color(cx.theme().foreground)
            .child(super::product_surface::body().child(content))
            .children(self.calibration_modal.clone())
    }
}
