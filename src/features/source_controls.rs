//! Retained native controls whose labels, bounds, options and bindings are
//! supplied by statically audited per-product descriptors.
use super::Choice;
use crate::ui::stepper::{Stepper, StepperEvent};
use crate::ui::surface;
use gpui_kit::component::{
    button::Button,
    checkbox::Checkbox,
    radio::Radio,
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::Value;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::OnceLock};

mod oled_page;
mod oled_presets;
mod oled_system_editor;
mod receiver;

#[derive(Deserialize)]
struct OptionSpec {
    label: String,
    value: Value,
    #[serde(default)]
    disabled_on_ble: bool,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    description: Option<String>,
}
#[derive(Deserialize)]
struct ConditionSpec {
    path: String,
    value: Value,
}
#[derive(Deserialize)]
struct MinimumSpec {
    path: String,
    value: Value,
    min: f32,
}
#[derive(Deserialize)]
struct ControlSpec {
    key: String,
    label: String,
    path: String,
    /// Source-specific renderer selected by the audited product descriptor.
    #[serde(default)]
    renderer: Option<String>,
    #[serde(default)]
    hide_label: bool,
    #[serde(default)]
    apply_label: Option<String>,
    #[serde(default)]
    complemented_byte: bool,
    #[serde(default)]
    disabled_on_ble: bool,
    kind: String,
    #[serde(default)]
    min: f32,
    #[serde(default)]
    max: f32,
    #[serde(default)]
    step: f32,
    #[serde(default)]
    options: Vec<OptionSpec>,
    #[serde(default)]
    disabled_when: Option<String>,
    #[serde(default)]
    disabled_unless: Option<String>,
    #[serde(default)]
    disabled_unless_all: Vec<String>,
    /// 任一条件组全部成立即禁用。原版摄像头的取景块用它表达
    /// `ldc && (4K 30FPS | 1440p 30FPS)`：屏幕上的变焦、平移/倾斜、预设与
    /// 快捷键同时进入原版的 `disabled` 状态并显示 LDC 说明。
    #[serde(default)]
    disabled_when_any: Vec<Vec<ConditionSpec>>,
    #[serde(default)]
    visible_when: Option<ConditionSpec>,
    #[serde(default)]
    reset_value: Option<Value>,
    #[serde(default)]
    minimum_when: Option<MinimumSpec>,
    #[serde(default)]
    enabled_from_value: Option<String>,
    /// Source setting-row tooltip text key.
    #[serde(default)]
    tooltip: Option<String>,
    /// Row description shown by the source inside its disabled branch.
    #[serde(default)]
    description: Option<String>,
    /// Idle text of a shortcut capture field.
    #[serde(default)]
    placeholder: Option<String>,
    /// Second bound field of a two-axis control (pan and tilt).
    #[serde(default)]
    tilt_path: Option<String>,
    /// 原版共享设置行的 `hasStepper` 标志，真值表示这一行带数字步进器。
    ///（校验脚本按 `名字：值` 的 ASCII 冒号形式识别字段，注释里不要那样写。）
    #[serde(default)]
    has_stepper: bool,
    /// Effective props of the actual numeric editor, after the setting-row adapter.
    #[serde(default)]
    allow_decimal: bool,
    #[serde(default)]
    round_up_decimals: bool,
    #[serde(default)]
    stepper_max_length: Option<usize>,
    #[serde(default = "default_max_pan_tilt")]
    max_pan_tilt: f32,
    /// Mounted pan/tilt pad content box, in the product's own pixels.
    #[serde(default)]
    box_width: f32,
    #[serde(default)]
    box_height: f32,
}
fn default_max_pan_tilt() -> f32 {
    10.
}
#[derive(Deserialize)]
struct SectionSpec {
    title: String,
    #[serde(default)]
    column: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    description: Option<String>,
    controls: Vec<ControlSpec>,
}
#[derive(Deserialize)]
struct PageSpec {
    key: String,
    sections: Vec<SectionSpec>,
}
#[derive(Deserialize)]
struct ProductSpec {
    product_id: u32,
    profile: Value,
    #[serde(default)]
    device_fields: Vec<String>,
    pages: Vec<PageSpec>,
    #[serde(default)]
    support: Option<String>,
    /// Current camera roots mount one 400px `.camera-container` column.
    #[serde(default)]
    layout: Option<String>,
}
fn specs() -> &'static [ProductSpec] {
    static SPECS: OnceLock<Vec<ProductSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        let mut specs: Vec<ProductSpec> =
            serde_json::from_str(include_str!("source_controls_data.json"))
                .expect("validated camera descriptors");
        specs.extend(
            serde_json::from_str::<Vec<ProductSpec>>(include_str!("keyboard_oled_data.json"))
                .expect("validated OLED descriptors"),
        );
        specs.extend(
            serde_json::from_str::<Vec<ProductSpec>>(include_str!("accessory_controls_data.json"))
                .expect("validated accessory descriptors"),
        );
        specs
    })
}
pub(crate) fn supports(pid: u32) -> bool {
    specs().iter().any(|s| s.product_id == pid)
}
pub(crate) fn device_fields(pid: u32) -> &'static [String] {
    specs()
        .iter()
        .find(|s| s.product_id == pid)
        .map_or(&[], |s| s.device_fields.as_slice())
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    specs()
        .iter()
        .find(|s| s.product_id == pid)
        .is_some_and(|s| {
            s.pages
                .iter()
                .any(|p| p.key == key && !p.sections.is_empty())
        })
}
pub(crate) struct SourceControlsChanged;
pub(crate) struct SourceControlsPairingRequested;
pub(crate) struct SourceControls {
    spec: &'static ProductSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    steppers: BTreeMap<String, Entity<crate::ui::stepper::Stepper>>,
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    staged: BTreeMap<String, Value>,
    is_ble: bool,
    focus: FocusHandle,
    /// Key of the shortcut capture field currently listening for input.
    listening: Option<String>,
    /// 原版黑框的 `getBoundingClientRect()`：由子元素 prepaint 写入内容盒坐标，
    /// 拖拽时用来把窗口坐标换成框内坐标。
    pan_tilt_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// 原版白框按下后记录的抓取偏移（`left/top/bottom/right`）。
    pan_tilt_drag: Option<PanTiltDrag>,
    receiver: receiver::ReceiverState,
}
/// 原版 `white box` 的 onMouseDown 状态：光标相对白框四条边的距离。
#[derive(Clone)]
struct PanTiltDrag {
    key: String,
    left: f32,
    top: f32,
    bottom: f32,
    right: f32,
}
impl EventEmitter<SourceControlsChanged> for SourceControls {}
impl EventEmitter<SourceControlsPairingRequested> for SourceControls {}
impl SourceControls {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = specs()
            .iter()
            .find(|s| s.product_id == pid)
            .expect("source controls registered");
        let mut this = Self {
            spec,
            page: spec.pages.first().map_or(String::new(), |p| p.key.clone()),
            draft: spec.profile.clone(),
            sliders: BTreeMap::new(),
            steppers: BTreeMap::new(),
            selects: BTreeMap::new(),
            subscriptions: vec![],
            syncing: false,
            staged: BTreeMap::new(),
            is_ble: false,
            focus: cx.focus_handle(),
            listening: None,
            pan_tilt_bounds: Rc::new(Cell::new(None)),
            pan_tilt_drag: None,
            receiver: receiver::ReceiverState::default(),
        };
        for control in spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            let key = control.key.clone();
            if control.kind == "slider" && !this.sliders.contains_key(&key) {
                let initial = this
                    .value(control)
                    .and_then(Value::as_f64)
                    .map(|v| v as f32)
                    .unwrap_or(control.min);
                let state = cx.new(|_| {
                    SliderState::new()
                        .min(control.min)
                        .max(control.max)
                        .step(control.step)
                        .default_value(initial)
                });
                let target = key.clone();
                this.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing {
                            return;
                        }
                        let SliderEvent::Change(value) = event else {
                            return;
                        };
                        this.edit(&target, serde_json::json!(value.start()), window, cx);
                    },
                ));
                this.sliders.insert(key.clone(), state);
            }
            // A source setting row can own both controls. Creating its slider
            // must not skip the separately retained numeric input.
            if control.has_stepper && !this.steppers.contains_key(&key) {
                // Slider and numeric editor write the same source setting.
                let initial = this.value(control).and_then(Value::as_f64).unwrap_or(0.);
                let (min, max, step) = (
                    control.min as f64,
                    control
                        .max
                        .to_string()
                        .parse::<f64>()
                        .expect("source maximum"),
                    control
                        .step
                        .to_string()
                        .parse::<f64>()
                        .expect("source step"),
                );
                let state = cx.new(|cx| {
                    Stepper::new(
                        SharedString::from(format!("{key}:stepper")),
                        initial,
                        (min, max, step),
                        control.allow_decimal,
                        control.round_up_decimals,
                        control.stepper_max_length,
                        window,
                        cx,
                    )
                });
                let target = key.clone();
                // 步进器写的是同一个字段，走和滑块一样的 `edit` 路径（含禁用判断）。
                this.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event: &StepperEvent, window, cx| {
                        if this.syncing {
                            return;
                        }
                        this.edit(&target, serde_json::json!(event.value), window, cx);
                    },
                ));
                this.steppers.insert(key, state);
            } else if control.kind == "select" && !this.selects.contains_key(&key) {
                let choices = control
                    .options
                    .iter()
                    .enumerate()
                    .map(|(ix, o)| Choice::new(ix.to_string(), crate::i18n::t(&o.label)))
                    .collect::<Vec<_>>();
                let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
                let target = key.clone();
                this.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing {
                            return;
                        }
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Some(value) = value
                                .parse::<usize>()
                                .ok()
                                .and_then(|ix| {
                                    this.control(&target).and_then(|c| c.options.get(ix))
                                })
                                .map(|o| o.value.clone())
                            {
                                this.select_value(&target, value, window, cx);
                            }
                        }
                    },
                ));
                this.selects.insert(key, state);
            }
        }
        this.sync(window, cx);
        this
    }
    fn control(&self, key: &str) -> Option<&'static ControlSpec> {
        self.spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
            .find(|c| c.key == key)
    }
    fn image_index(&self) -> usize {
        self.draft
            .get("imageMode")
            .or_else(|| self.draft.get("imageSettingMode"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize
    }
    fn path(&self, control: &ControlSpec) -> String {
        self.resolve_path(&control.path)
    }
    fn resolve_path(&self, path: &str) -> String {
        if let Some(field) = path.strip_prefix("@image/") {
            format!("/image/{}/dataSet/{field}", self.image_index())
        } else if let Some(field) = path.strip_prefix("@view/") {
            let mode = self.draft.pointer("/camera/viewPresets/viewMode");
            let ix = self
                .draft
                .pointer("/camera/viewPresets/data")
                .and_then(Value::as_array)
                .and_then(|presets| presets.iter().position(|p| p.get("mode") == mode));
            ix.map(|ix| format!("/camera/viewPresets/data/{ix}/{field}"))
                .unwrap_or_else(|| "/missing-view-preset".into())
        } else {
            path.to_owned()
        }
    }
    fn value(&self, control: &ControlSpec) -> Option<&Value> {
        self.draft.pointer(&self.path(control))
    }
    fn selection_value(&self, control: &ControlSpec) -> Option<Value> {
        self.staged.get(&control.key).cloned().or_else(|| {
            self.value(control).map(|value| {
                if control.complemented_byte {
                    if let Some(raw) = value.as_u64().filter(|raw| *raw <= 255) {
                        return Value::from(if raw < 127 { raw } else { (!raw) & 255 });
                    }
                }
                value.clone()
            })
        })
    }
    fn select_value(
        &mut self,
        key: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(control) = self.control(key) else {
            return;
        };
        if self.disabled(control)
            || !control
                .options
                .iter()
                .any(|o| o.value == value && !(o.disabled_on_ble && self.is_ble))
        {
            return;
        }
        if control.apply_label.is_some() {
            self.staged.insert(key.into(), value);
            cx.notify();
        } else {
            self.edit(key, value, window, cx);
        }
    }
    pub(crate) fn set_connection(&mut self, is_ble: bool, cx: &mut Context<Self>) {
        self.is_ble = is_ble;
        cx.notify();
    }
    fn minimum(&self, control: &ControlSpec) -> f32 {
        control
            .minimum_when
            .as_ref()
            .filter(|condition| {
                self.draft.pointer(&self.resolve_path(&condition.path)) == Some(&condition.value)
            })
            .map_or(control.min, |condition| condition.min)
    }
    fn disabled(&self, control: &ControlSpec) -> bool {
        self.value(control).is_none()
            || (control.disabled_on_ble && self.is_ble)
            || control.disabled_when.as_ref().is_some_and(|path| {
                self.draft
                    .pointer(&self.resolve_path(path))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            || control.disabled_unless.as_ref().is_some_and(|path| {
                self.draft
                    .pointer(&self.resolve_path(path))
                    .and_then(Value::as_bool)
                    != Some(true)
            })
            || control.disabled_unless_all.iter().any(|path| {
                self.draft
                    .pointer(&self.resolve_path(path))
                    .and_then(Value::as_bool)
                    != Some(true)
            })
            || control.disabled_when_any.iter().any(|group| {
                group.iter().all(|condition| {
                    self.draft.pointer(&self.resolve_path(&condition.path))
                        == Some(&condition.value)
                })
            })
    }
    /// 原版黑框的 `onMouseMove`：按住白框后在黑框上拖动，按抓取偏移移动白框，
    /// 依次套用左右/上下边界钳制，再把像素位置换算回 pan/tilt。
    fn drag_pan_tilt(
        &mut self,
        key: &str,
        box_size: &PanTiltBox,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.pan_tilt_drag.clone() else {
            return;
        };
        if drag.key != key {
            return;
        }
        let Some(bounds) = self.pan_tilt_bounds.get() else {
            return;
        };
        let Some(control) = self.control(key) else {
            return;
        };
        let pan_path = control.path.clone();
        let tilt_path = control.tilt_path.clone().unwrap_or_default();
        let box_size = PanTiltBox {
            width: box_size.width,
            height: box_size.height,
            zoom: box_size.zoom,
            max: box_size.max,
        };
        let white_w = box_size.width / box_size.zoom;
        let white_h = box_size.height / box_size.zoom;
        let outer_w = box_size.width + 2.;
        let outer_h = box_size.height + 2.;
        // 白框已经和黑框一样大时不再响应（原版的提前返回）。
        if (outer_w - white_w).abs() < f32::EPSILON && (outer_h - white_h).abs() < f32::EPSILON {
            return;
        }
        let (current_left, current_top) =
            box_size.white_origin(self.axis_value(&pan_path), self.axis_value(&tilt_path));
        let x = f32::from(position.x - bounds.origin.x);
        let y = f32::from(position.y - bounds.origin.y);
        let mut left = x - drag.left;
        let mut top = y - drag.top;
        let bottom = y + drag.bottom;
        let right = x + drag.right;
        if left < 0. {
            left = 0.;
        }
        if top < 0. {
            top = 0.;
        }
        if bottom >= outer_h {
            top = outer_h - white_h - 2.;
        }
        if right >= outer_w {
            left = outer_w - white_w - 2.;
        }
        if left == current_left && top == current_top {
            return;
        }
        let pan = box_size.pan_from_left(left, white_w);
        let tilt = box_size.tilt_from_top(top, white_h);
        // 原版在这里还把像素位置钳进内容盒，但那只写进本地 state，pan/tilt 用的是
        // 钳制之前的值；本地白框位置每次都由 `white_origin` 重算，因此只写值。
        self.write_path(&pan_path, serde_json::json!(pan), window, cx);
        self.write_path(&tilt_path, serde_json::json!(tilt), window, cx);
    }

    fn edit(&mut self, key: &str, mut value: Value, window: &mut Window, cx: &mut Context<Self>) {
        let Some(control) = self.control(key) else {
            return;
        };
        if self.disabled(control)
            || control.visible_when.as_ref().is_some_and(|condition| {
                self.draft.pointer(&self.resolve_path(&condition.path)) != Some(&condition.value)
            })
        {
            return;
        }
        if control.kind == "reset" {
            let Some(reset) = &control.reset_value else {
                return;
            };
            value = reset.clone();
        }
        if matches!(control.kind.as_str(), "toggle" | "switch") && control.options.len() == 2 {
            value = control.options[usize::from(value.as_bool().unwrap_or(false))]
                .value
                .clone();
        }
        if control.kind == "slider" {
            let Some(number) = value.as_f64() else {
                return;
            };
            if !number.is_finite() || control.step <= 0. {
                return;
            }
            let min = self.minimum(control);
            let n = (number as f32).clamp(min, control.max);
            let snapped =
                (min + ((n - min) / control.step).round() * control.step).clamp(min, control.max);
            value = if control.step.fract() == 0. {
                serde_json::json!(snapped as i64)
            } else if control.has_stepper {
                // Camera editors accept at most three decimal places. Do not
                // expose the slider's f32 conversion tail in the text input.
                serde_json::json!((f64::from(snapped) * 1000.).round() / 1000.)
            } else {
                serde_json::json!(snapped)
            };
        }
        if matches!(
            control.kind.as_str(),
            "select" | "options" | "image_options" | "preset"
        ) && !control
            .options
            .iter()
            .any(|option| option.value == value && !(option.disabled_on_ble && self.is_ble))
        {
            return;
        }
        if self.value(control) == Some(&value) {
            return;
        }
        if control.path.starts_with("@image/") {
            let current = self.image_index();
            let custom = self
                .draft
                .get("image")
                .and_then(Value::as_array)
                .and_then(|a| {
                    a.iter()
                        .position(|p| p.get("name").and_then(Value::as_str) == Some("custom"))
                });
            if let Some(custom) = custom {
                if current != custom {
                    let source = self
                        .draft
                        .pointer(&format!("/image/{current}/dataSet"))
                        .cloned();
                    if let Some(source) = source {
                        self.draft["image"][custom]["dataSet"] = source;
                    }
                    let mode = if self.draft.get("imageMode").is_some() {
                        "imageMode"
                    } else {
                        "imageSettingMode"
                    };
                    self.draft[mode] = serde_json::json!(custom);
                }
            }
        }
        let path = self.path(control);
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = value.clone();
        }
        if let Some(enabled) = &control.enabled_from_value {
            let enabled = self.resolve_path(enabled);
            if let Some(target) = self.draft.pointer_mut(&enabled) {
                *target = Value::Bool(value.as_f64().is_some_and(|v| v != 0.));
            }
        }
        self.normalize();
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        cx.notify();
    }
    /// Writes one resolved path directly. Multi-field controls (pan and tilt,
    /// the preset shortcut chord) own more than one bound field.
    fn write_path(
        &mut self,
        path: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = self.resolve_path(path);
        let Some(target) = self.draft.pointer_mut(&path) else {
            return;
        };
        if *target == value {
            return;
        }
        *target = value;
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        cx.notify();
    }
    fn axis_value(&self, path: &str) -> f32 {
        self.draft
            .pointer(&self.resolve_path(path))
            .and_then(Value::as_f64)
            .unwrap_or(0.) as f32
    }
    /// Nudges one pan/tilt axis by the pad's own single step, bounded by the
    /// mounted `maxPanTilt`. The centre button re-centres both axes.
    fn nudge_axis(
        &mut self,
        control: &ControlSpec,
        path: &str,
        delta: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.axis_value(path);
        let next = (current + delta).clamp(-control.max_pan_tilt, control.max_pan_tilt);
        if next == current {
            return;
        }
        self.write_path(path, serde_json::json!(next), window, cx);
    }
    /// Source listener: a chord is retained only when it carries both a
    /// modifier and a key; a lone key is presented as Ctrl + Shift + key.
    fn capture_shortcut(
        &mut self,
        control: &ControlSpec,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut ids: Vec<String> = Vec::new();
        for (pressed, id) in [
            (event.keystroke.modifiers.control, "KEY_LEFT_CTRL"),
            (event.keystroke.modifiers.shift, "KEY_LEFT_SHIFT"),
            (event.keystroke.modifiers.alt, "KEY_LEFT_ALT"),
            (event.keystroke.modifiers.platform, "KEY_LEFT_GUI"),
        ] {
            if pressed {
                ids.push(id.into());
            }
        }
        let Some(key) = keystroke_input_id(&event.keystroke.key) else {
            return;
        };
        if !ids.iter().any(|id| id == key) {
            ids.push(key.into());
        }
        if ids.len() == 1 {
            ids.insert(0, "KEY_LEFT_SHIFT".into());
            ids.insert(0, "KEY_LEFT_CTRL".into());
        }
        let keeps = ids.iter().any(|id| is_modifier(id)) && ids.iter().any(|id| !is_modifier(id));
        let value = if keeps {
            Value::Array(ids.into_iter().map(Value::from).collect())
        } else {
            Value::Array(Vec::new())
        };
        self.write_path(&control.path.clone(), value, window, cx);
    }
    fn normalize(&mut self) {
        for control in self
            .spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            // Disabled source fields can retain sentinel values (3907's idle
            // minutes starts at zero). Clamp only when the control is active.
            if control.kind != "slider" || self.disabled(control) {
                continue;
            }
            let path = self.path(control);
            let min = self.minimum(control);
            if let Some(value) = self.draft.pointer_mut(&path).filter(|v| v.is_number()) {
                let number = value.as_f64().unwrap_or(min as f64) as f32;
                let number = (min
                    + ((number.clamp(min, control.max) - min) / control.step).round()
                        * control.step)
                    .clamp(min, control.max);
                *value = if control.step.fract() == 0. {
                    serde_json::json!(number as i64)
                } else if control.has_stepper {
                    serde_json::json!((f64::from(number) * 1000.).round() / 1000.)
                } else {
                    serde_json::json!(number)
                };
            }
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (key, state) in &self.sliders {
            if let Some(control) = self.control(key) {
                if let Some(value) = self.value(control).and_then(Value::as_f64) {
                    let min = self.minimum(control);
                    state.update(cx, |s, cx| {
                        if s.min_value() != min {
                            *s = SliderState::new()
                                .min(min)
                                .max(control.max)
                                .step(control.step);
                        }
                        s.set_value(value as f32, window, cx);
                    });
                }
            }
        }
        for (key, state) in &self.steppers {
            if let Some(control) = self.control(key) {
                let value = self.value(control).and_then(Value::as_f64).unwrap_or(0.);
                let disabled = self.disabled(control);
                let min = f64::from(self.minimum(control));
                state.update(cx, |stepper, cx| {
                    stepper.sync_value(value, min, disabled, window, cx)
                });
            }
        }
        for (key, state) in &self.selects {
            if let Some(control) = self.control(key) {
                let value = self.selection_value(control);
                let selected = control
                    .options
                    .iter()
                    .position(|o| Some(&o.value) == value.as_ref())
                    .map(|i| i.to_string());
                state.update(cx, |s, cx| {
                    s.set_selected_value(&selected.unwrap_or_default(), window, cx)
                });
            }
        }
        self.syncing = false;
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
        let previous_indicator = self.draft.pointer("/runtime/indicatorLedStatus").cloned();
        self.draft = self.spec.profile.clone();
        self.staged.clear();
        if let Some(value) = value {
            merge_known(&mut self.draft, value);
            self.restore_oled_custom_presets(value);
        }
        self.normalize();
        if self.spec.product_id == 179
            && previous_indicator.as_ref() != self.draft.pointer("/runtime/indicatorLedStatus")
        {
            self.receiver.restart_indicator();
        }
        self.sync(window, cx);
        cx.notify();
    }
    pub(crate) fn set_page(&mut self, page: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            if self.spec.product_id == 179 {
                self.receiver.restart_indicator();
            }
            self.staged.clear();
            self.sync(window, cx);
        }
        self.page = page.into();
        cx.notify();
    }
    /// `.pan-and-tilt-container`: a 220x132 content box with a 1px border, the
    /// white framing box scaled by zoom, and the five pad buttons.
    fn render_pan_tilt(
        &self,
        control: &ControlSpec,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::theme::CameraProductColors as Colors;
        let key = control.key.clone();
        let tilt_path = control.tilt_path.clone().unwrap_or_default();
        let pan = self.axis_value(&control.path);
        let tilt = self.axis_value(&tilt_path);
        let zoom = self.axis_value("@view/zoom").max(1.);
        let lines = PanTiltBox {
            width: control.box_width,
            height: control.box_height,
            zoom,
            max: control.max_pan_tilt,
        };
        let (left, top) = lines.white_origin(pan, tilt);
        let outer_w = control.box_width + 2.;
        let outer_h = control.box_height + 2.;
        let white_w = control.box_width / zoom;
        let white_h = control.box_height / zoom;
        let pad_button = |id: &str,
                          delta: f32,
                          axis_tilt: bool,
                          x: f32,
                          y: f32,
                          size: f32,
                          hidden: u8,
                          icon: Option<&'static str>|
         -> AnyElement {
            let control_key = key.clone();
            let path = if axis_tilt {
                tilt_path.clone()
            } else {
                control.path.clone()
            };
            let mut button = div()
                .id(SharedString::from(format!("{}:{id}", control.key)))
                .absolute()
                .left(surface::css(x))
                .top(surface::css(y))
                .w(surface::css(size))
                .h(surface::css(size))
                .rounded(surface::css(2.))
                .border_1()
                .border_color(Colors::border());
            // The pad hides the border facing the black box (CSS per side).
            button = match hidden {
                0 => button.border_b_0(),
                1 => button.border_l_0(),
                2 => button.border_t_0(),
                3 => button.border_r_0(),
                _ => button,
            };
            // 原版方向键使用产品包中的 pan-top/pan-bottom 以及左右箭头图标。
            // 图标尺寸与 CSS 的 10px background-size 对齐；中心键使用 20px 画布，
            // 其透明边距正好把 15px 的可点击区域包住。
            if let Some(icon) = icon {
                button = button.overflow_hidden().child(
                    img(icon)
                        .absolute()
                        .left(surface::css(1.))
                        .top(surface::css(1.))
                        .w(surface::css(size))
                        .h(surface::css(size)),
                );
            }
            button
                .when(!disabled, |button| {
                    button
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(control) = this.control(&control_key) {
                                this.nudge_axis(control, &path, delta, window, cx);
                            }
                        }))
                })
                .into_any_element()
        };
        let mut pad = div()
            .relative()
            .w(surface::css(outer_w))
            .h(surface::css(outer_h))
            .mx_auto()
            .my(surface::css(20.))
            // 原版白框是黑框的子元素，因此先画黑框、再画白框。
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    // 原版 `.black-box` 是 `box-sizing: content-box` 的 220x132 加
                    // 1px 边框，因此边框盒是 222x134，白框坐标相对内容盒。
                    .w(surface::css(outer_w))
                    .h(surface::css(outer_h))
                    .border_1()
                    .border_color(Colors::border())
                    .when(!disabled, |black_box| {
                        let move_key = key.clone();
                        let drag = PanTiltBox {
                            width: control.box_width,
                            height: control.box_height,
                            zoom,
                            max: control.max_pan_tilt,
                        };
                        black_box.on_mouse_move(cx.listener(
                            move |this, event: &MouseMoveEvent, window, cx| {
                                this.drag_pan_tilt(&move_key, &drag, event.position, window, cx);
                            },
                        ))
                    })
                    // 内容盒（220x132）的窗口坐标：布局阶段写入，供拖拽换算指针位置。
                    .child({
                        let slot = self.pan_tilt_bounds.clone();
                        canvas(
                            move |bounds, _, _| {
                                slot.set(Some(bounds));
                                bounds
                            },
                            |_, _, _, _| {},
                        )
                        .size_full()
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{}:white-box", control.key)))
                    .absolute()
                    // 原版白框是黑框的子元素，坐标相对黑框的 padding 盒；本地把两者
                    // 都绝对定位在同一个容器里，因此补上黑框 1px 边框的偏移。
                    .left(surface::css(left + 1.))
                    .top(surface::css(top + 1.))
                    .w(surface::css(white_w))
                    .h(surface::css(white_h))
                    .bg(Colors::text())
                    // 原版：按下白框记录抓取偏移，抬起时清零。
                    .when(!disabled, |white_box| {
                        let down_key = key.clone();
                        let up_key = key.clone();
                        let drag = PanTiltBox {
                            width: control.box_width,
                            height: control.box_height,
                            zoom,
                            max: control.max_pan_tilt,
                        };
                        white_box
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    let Some(bounds) = this.pan_tilt_bounds.get() else {
                                        return;
                                    };
                                    let (left, top) = drag.white_origin(pan, tilt);
                                    let x = f32::from(event.position.x - bounds.origin.x);
                                    let y = f32::from(event.position.y - bounds.origin.y);
                                    this.pan_tilt_drag = Some(PanTiltDrag {
                                        key: down_key.clone(),
                                        left: x - left,
                                        top: y - top,
                                        bottom: white_h - (y - top),
                                        right: white_w - (x - left),
                                    });
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    if this
                                        .pan_tilt_drag
                                        .as_ref()
                                        .is_some_and(|drag| drag.key == up_key)
                                    {
                                        this.pan_tilt_drag = None;
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }),
                            )
                    }),
            );
        pad = pad
            .child(pad_button(
                "up",
                1.,
                true,
                outer_w / 2. - 5.,
                -10.,
                10.,
                0,
                Some("synapse/camera-pan-top.svg"),
            ))
            .child(pad_button(
                "right",
                1.,
                false,
                outer_w,
                outer_h / 2. - 5.,
                10.,
                1,
                Some("synapse/history-forward.svg"),
            ))
            .child(pad_button(
                "down",
                -1.,
                true,
                outer_w / 2. - 5.,
                outer_h,
                10.,
                2,
                Some("synapse/camera-pan-bottom.svg"),
            ))
            .child(pad_button(
                "left",
                -1.,
                false,
                -10.,
                outer_h / 2. - 5.,
                10.,
                3,
                Some("synapse/history-back.svg"),
            ))
            .child(
                div()
                    .id(SharedString::from(format!("{}:center", control.key)))
                    .absolute()
                    .left(surface::css(outer_w / 2. - 7.5))
                    .top(surface::css(outer_h / 2. - 7.5))
                    .w(surface::css(15.))
                    .h(surface::css(15.))
                    .rounded(surface::css(2.))
                    .when(!disabled, |center| {
                        center
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.write_path("@view/pan", serde_json::json!(0), window, cx);
                                this.write_path("@view/tilt", serde_json::json!(0), window, cx);
                            }))
                    })
                    .when_some(control.tooltip.as_ref(), |center, tooltip| {
                        let tooltip = crate::i18n::t(tooltip);
                        center.tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tooltip.clone())
                                .build(window, cx)
                        })
                    })
                    .overflow_hidden()
                    .child(
                        img("synapse/camera-pan-center.svg")
                            .absolute()
                            .left(surface::css(-2.5))
                            .top(surface::css(-2.5))
                            .w(surface::css(20.))
                            .h(surface::css(20.)),
                    ),
            );
        v_flex()
            .gap(surface::css(4.))
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .child(crate::i18n::t(&control.label)),
            )
            .child(pad)
            .into_any_element()
    }
    /// `.display-name.keyboard_listen`: the preset shortcut capture field.
    fn render_shortcut_key(&self, control: &ControlSpec, cx: &mut Context<Self>) -> AnyElement {
        use crate::ui::theme::CameraProductColors as Colors;
        let key = control.key.clone();
        let listening = self.listening.as_deref() == Some(control.key.as_str());
        let captured = self
            .value(control)
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(Value::as_str)
                    .map(input_label)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut field = div()
            .id(SharedString::from(format!("{}:capture", control.key)))
            .track_focus(&self.focus)
            .w(surface::css(210.))
            .px(surface::css(5.))
            .py(surface::css(5.))
            .bg(Colors::background())
            .border_1()
            .border_color(if listening {
                cx.theme().primary
            } else {
                Colors::border()
            })
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(Colors::text())
            .cursor_pointer()
            // `.display-name.keyboard-listen:hover/.active` turns the border
            // green; the listening state keeps it there.
            .hover(|field| field.border_color(cx.theme().primary))
            .on_click(cx.listener({
                let listen_key = key.clone();
                move |this, _, window, cx| {
                    this.listening = Some(listen_key.clone());
                    this.focus.focus(window, cx);
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let Some(key) = this.listening.clone() else {
                    return;
                };
                if let Some(control) = this.control(&key) {
                    this.capture_shortcut(control, event, window, cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.listening.take().is_some() {
                    cx.notify();
                }
            }));
        if captured.is_empty() {
            field = field.child(
                div()
                    .text_color(Colors::placeholder())
                    .child(crate::i18n::t(
                        control.placeholder.as_deref().unwrap_or_default(),
                    )),
            );
        } else {
            let clear_key = key.clone();
            field = field
                .child(div().child(captured.join(" + ")))
                .child(
                    div()
                        .id(SharedString::from(format!("{}:clear", control.key)))
                        .absolute()
                        .right_0()
                        .top_0()
                        .px(surface::css(4.))
                        .text_color(Colors::text())
                        .child("×")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let path = this
                                .control(&clear_key)
                                .map(|control| control.path.clone())
                                .unwrap_or_default();
                            this.write_path(&path, Value::Array(Vec::new()), window, cx);
                            this.listening = None;
                        })),
                )
                .relative();
        }
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .child(crate::i18n::t(&control.label)),
            )
            .child(field)
            .into_any_element()
    }
    fn render_control(
        &self,
        control: &ControlSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if control.visible_when.as_ref().is_some_and(|condition| {
            self.draft.pointer(&self.resolve_path(&condition.path)) != Some(&condition.value)
        }) {
            return div().into_any_element();
        }
        let key = control.key.clone();
        let label = crate::i18n::t(&control.label);
        if control.renderer.as_deref() == Some("hyperpolling-pairing") {
            return self.render_hyperpolling_pairing(cx);
        }
        let disabled = self.disabled(control);
        if control.renderer.as_deref() == Some("indicator-radio") {
            return self.render_indicator_radio(control, disabled, cx);
        }
        match control.kind.as_str() {
            "oled_presets" => self.render_oled_presets(disabled, window, cx),
            // `.preset-container .preset-item`: 50px grid columns, 27px tall
            // numbered squares, selected = #292929 on a #44d62c border.
            "preset" => {
                use crate::ui::theme::CameraProductColors as Colors;
                v_flex()
                    .gap_2()
                    .when(!control.hide_label, |view| view.child(label))
                    .when(disabled, |view| {
                        view.when_some(control.description.as_ref(), |view, description| {
                            view.child(
                                div()
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(17.))
                                    .text_color(Colors::text())
                                    // `.mode-description{margin-top:10px}`；外层是 8px 的
                                    // flex 间距，因此这里补 2px 才是原版的 10px。
                                    .mt(surface::css(2.))
                                    .child(crate::i18n::t(description)),
                            )
                        })
                    })
                    .child(h_flex().flex_wrap().gap(surface::css(10.)).children(
                        control.options.iter().map(|option| {
                            let selected = self.value(control) == Some(&option.value);
                            let value = option.value.clone();
                            let key = key.clone();
                            gpui_kit::base::Button::new(SharedString::from(format!(
                                "{}:{}",
                                key, value
                            )))
                            .accessibility_label(crate::i18n::t(&option.label))
                            .disabled(disabled)
                            .w(surface::css(50.))
                            .h(surface::css(27.))
                            .px(surface::css(16.))
                            .pt(surface::css(6.))
                            .pb(surface::css(6.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .text_center()
                            .text_color(Colors::text())
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                Colors::border()
                            })
                            .when(selected, |b| b.bg(Colors::selected()))
                            .styles(|s| s.disabled(|s| s.opacity(0.3)))
                            .focus_visible(|b| b.border_color(cx.theme().primary))
                            .child(crate::i18n::t(&option.label))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.edit(&key, value.clone(), window, cx)
                                },
                            ))
                        }),
                    ))
                    .into_any_element()
            }
            "pan_tilt" => self.render_pan_tilt(control, disabled, cx),
            // `.direction-container .direction-item`: 48x27 swatches with a
            // 20x3 placement line that turns green for the current position.
            "direction" => {
                use crate::ui::theme::CameraProductColors as Colors;
                v_flex()
                    .gap_2()
                    .when_some(control.description.as_ref(), |view, description| {
                        view.child(
                            div()
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .text_color(Colors::text())
                                .child(crate::i18n::t(description)),
                        )
                    })
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(surface::css(6.))
                            .mt(surface::css(9.))
                            .children(control.options.iter().map(|option| {
                                let selected = self.value(control) == Some(&option.value);
                                let value = option.value.clone();
                                let key = key.clone();
                                let name = option.value.as_str().unwrap_or_default().to_string();
                                let accent = cx.theme().primary;
                                // `.direction-line.<position>` offsets, from the
                                // per-placement CSS rules (48x27 item, 20x3 line).
                                let (left, right, top, bottom) = match name.as_str() {
                                    "left-bottom" => (Some(0.), None, None, Some(0.)),
                                    "right-bottom" => (None, Some(0.), None, Some(0.)),
                                    "center-bottom" => (Some(14.), None, None, Some(0.)),
                                    "left-top" => (Some(0.), None, Some(0.), None),
                                    "right-top" => (None, Some(0.), Some(0.), None),
                                    _ => (Some(14.), None, Some(0.), None),
                                };
                                div()
                                    .id(SharedString::from(format!("{key}:{name}")))
                                    .relative()
                                    .w(surface::css(48.))
                                    .h(surface::css(27.))
                                    .bg(Colors::swatch())
                                    .when(!disabled, |swatch| {
                                        swatch.cursor_pointer().on_click(cx.listener(
                                            move |this, _, window, cx| {
                                                this.edit(&key, value.clone(), window, cx)
                                            },
                                        ))
                                    })
                                    .child(
                                        div()
                                            .absolute()
                                            .w(surface::css(20.))
                                            .h(surface::css(3.))
                                            .bg(if selected { accent } else { Colors::text() })
                                            .when_some(left, |line, left| {
                                                line.left(surface::css(left))
                                            })
                                            .when_some(right, |line, right| {
                                                line.right(surface::css(right))
                                            })
                                            .when_some(top, |line, top| line.top(surface::css(top)))
                                            .when_some(bottom, |line, bottom| {
                                                line.bottom(surface::css(bottom))
                                            }),
                                    )
                            })),
                    )
                    .when(disabled, |view| view.opacity(0.3))
                    .into_any_element()
            }
            "keys" => self.render_shortcut_key(control, cx),
            "image_options" => {
                use crate::ui::theme::OledColors;
                h_flex()
                    .w(surface::css(530.))
                    .flex_wrap()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .children(control.options.iter().map(|option| {
                        let selected = self.value(control) == Some(&option.value);
                        let value = option.value.clone();
                        let key = key.clone();
                        gpui_kit::base::Button::new(SharedString::from(format!("{key}:{value}")))
                            .accessibility_label(crate::i18n::t(&option.label))
                            .disabled(disabled)
                            .w(surface::css(260.))
                            .h(surface::css(68.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(OledColors::screen())
                            .text_color(OledColors::muted())
                            .text_size(surface::css(14.))
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                OledColors::border()
                            })
                            .when(selected, |button| button.border_2())
                            .when(!selected, |button| {
                                button.border_1().hover(|button| {
                                    button.border_2().border_color(OledColors::hover_border())
                                })
                            })
                            .focus_visible(|button| button.border_color(cx.theme().primary))
                            .child(if let Some(image) = &option.image {
                                img(SharedString::from(image.clone()))
                                    .w(surface::css(256.))
                                    .h(surface::css(64.))
                                    .object_fit(ObjectFit::Contain)
                                    .into_any_element()
                            } else {
                                div()
                                    .child(format!("({})", crate::i18n::t(&option.label)))
                                    .into_any_element()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(&key, value.clone(), window, cx)
                            }))
                    }))
                    .into_any_element()
            }
            "options" => v_flex()
                .gap_2()
                .when(!control.hide_label, |view| view.child(label))
                .child(h_flex().gap(surface::css(10.)).flex_wrap().children(
                    control.options.iter().map(|option| {
                        use crate::ui::theme::CameraProductColors as Colors;
                        let disabled = disabled || (option.disabled_on_ble && self.is_ble);
                        let selected = self.value(control) == Some(&option.value);
                        let value = option.value.clone();
                        let key = key.clone();
                        gpui_kit::base::Button::new(SharedString::from(format!(
                            "{}:{}",
                            key, value
                        )))
                        .accessibility_label(crate::i18n::t(&option.label))
                        .disabled(disabled)
                        .min_w(surface::css(90.))
                        .px(surface::css(16.))
                        .pt(surface::css(7.))
                        .pb(surface::css(6.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .text_color(Colors::text())
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(if selected {
                            cx.theme().primary
                        } else {
                            Colors::border()
                        })
                        .when(selected, |b| b.bg(Colors::selected()))
                        .styles(|s| s.disabled(|s| s.opacity(0.3)))
                        .when(!disabled, |b| {
                            b.hover(|b| b.bg(Colors::hover()).border_color(cx.theme().primary))
                                .active(|b| b.bg(Colors::pressed()))
                        })
                        .focus_visible(|b| b.border_color(cx.theme().primary))
                        .child(crate::i18n::t(&option.label))
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.edit(&key, value.clone(), window, cx),
                        ))
                    }),
                ))
                .into_any_element(),
            "reset" => {
                Button::new(SharedString::from(key.clone()))
                    .label(label)
                    .outline()
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(&key, Value::Null, window, cx)
                    }))
                    .into_any_element()
            }
            "switch" => surface::SynapseSwitch::new(SharedString::from(key.clone()))
                .label(label)
                .checked(if control.options.len() == 2 {
                    self.value(control) == Some(&control.options[1].value)
                } else {
                    self.value(control)
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .disabled(disabled)
                .on_change(cx.listener(move |this, value, window, cx| {
                    this.edit(&key, Value::Bool(*value), window, cx)
                }))
                .into_any_element(),
            "toggle" => Checkbox::new(SharedString::from(key.clone()))
                .label(label)
                .checked(if control.options.len() == 2 {
                    self.value(control) == Some(&control.options[1].value)
                } else {
                    self.value(control)
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .disabled(disabled)
                .on_click(cx.listener(move |this, value, window, cx| {
                    this.edit(&key, Value::Bool(*value), window, cx)
                }))
                .into_any_element(),
            "slider" => {
                // 原版设置行把步进器放在标题行右侧，内容区才是滑块。
                let stepper = control.has_stepper.then(|| self.steppers[&key].clone());
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .child(label)
                            .child(div().flex_1())
                            .when_some(stepper, |row, stepper| row.child(stepper)),
                    )
                    .child(Slider::new(&self.sliders[&key]).disabled(disabled))
                    .into_any_element()
            }
            "select" => v_flex()
                .gap_2()
                .when(!control.hide_label, |view| view.child(label))
                .child(
                    h_flex()
                        .gap_3()
                        .child(Select::new(&self.selects[&key]).disabled(disabled))
                        .when_some(control.apply_label.as_ref(), |view, label| {
                            let selected = self.selection_value(control);
                            let unchanged = selected.as_ref() == self.value(control);
                            view.child(
                                Button::new(SharedString::from(format!("{key}:apply")))
                                    .label(crate::i18n::t(label))
                                    .disabled(disabled || unchanged || selected.is_none())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(value) = this
                                            .control(&key)
                                            .and_then(|control| this.selection_value(control))
                                        {
                                            this.edit(&key, value, window, cx);
                                        }
                                    })),
                            )
                        }),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
    /// `.advanced-camera-container .camera-container`: one 400px `#111` column
    /// with 27px/20px padding and a `.camera-divider` between mounted rows.
    fn render_camera_column(
        &self,
        page: &'static PageSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::theme::CameraProductColors as Colors;
        let mut column = v_flex()
            .id("source-camera-settings-column")
            .w(surface::css(400.))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(Colors::background())
            .px(surface::css(20.))
            .py(surface::css(27.));
        for (index, section) in page.sections.iter().enumerate() {
            if index > 0 {
                column = column.child(
                    div()
                        .w_full()
                        .my(surface::css(20.))
                        .border_1()
                        .border_color(Colors::divider())
                        .rounded(surface::css(2.)),
                );
            }
            column = column.child(
                v_flex()
                    .gap(surface::css(10.))
                    .when(!section.title.is_empty(), |view| {
                        view.child(
                            div()
                                .text_size(surface::css(14.))
                                .line_height(surface::css(16.))
                                .child(crate::i18n::t(&section.title)),
                        )
                    })
                    .children(
                        section
                            .controls
                            .iter()
                            .map(|control| self.render_control(control, window, cx)),
                    ),
            );
        }
        column.into_any_element()
    }

    /// The audited accessory indicator renderer uses the shared radio-item
    /// contract. Generic source-options buttons do not match that renderer.
    fn render_indicator_radio(
        &self,
        control: &ControlSpec,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = control.key.clone();
        v_flex()
            .gap(surface::css(10.))
            .children(control.options.iter().map(|option| {
                let value = option.value.clone();
                let selected = self.value(control) == Some(&value);
                let option_key = key.clone();
                let description = option.description.clone();
                v_flex()
                    .gap(surface::css(4.))
                    .child(
                        Radio::new(SharedString::from(format!("{option_key}:{value}")))
                            .label(crate::i18n::t(&option.label))
                            .checked(selected)
                            .disabled(disabled)
                            .on_change(cx.listener(move |this, _, window, cx| {
                                this.edit(&option_key, value.clone(), window, cx)
                            })),
                    )
                    .when_some(description, |view, text| {
                        view.child(div().ml(surface::css(30.)).child(crate::i18n::t(&text)))
                    })
            }))
            .when(disabled, |view| view.opacity(0.3))
            .into_any_element()
    }

    /// Current product sources mount the same empty HyperPolling utility row:
    /// a 44px pairing glyph and an underlined `OPEN_PAIRING_UTILITY` action.
    /// The descriptor selects this renderer; no product id is consulted here.
    fn render_hyperpolling_pairing(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("source-hyperpolling-pairing")
            .flex()
            .items_center()
            .gap(surface::css(10.))
            .cursor_pointer()
            .on_click(cx.listener(|_, _, _, cx| {
                cx.emit(SourceControlsPairingRequested);
            }))
            .child(
                img("synapse/hyperpolling-icon-multidevicepairing2.svg")
                    .w(surface::css(44.))
                    .h(surface::css(44.)),
            )
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(44.))
                    .text_color(gpui_kit::rgb(0xcccccc))
                    .underline()
                    .hover(|view| view.text_color(gpui_kit::rgb(0x44d62c)))
                    .child(crate::i18n::t("OPEN_PAIRING_UTILITY")),
            )
            .into_any_element()
    }

    /// Legacy Kiyo roots (3587/3589/3590) mount the shared Customize widget,
    /// whose audited source still includes `.camera_setting .main_preview`.
    /// The source frame is 520px wide by 292px high with a `#222` surface.
    /// Camera transport is not connected here, so no video frame or device
    /// identity is fabricated.
    fn render_legacy_camera_preview(&self) -> AnyElement {
        use crate::ui::theme::CameraProductColors as Colors;
        v_flex()
            .id("legacy-camera-preview-unavailable")
            .test_support()
            .w(surface::css(520.))
            .h(surface::css(292.))
            .mx_auto()
            .bg(gpui_kit::rgb(0x222222))
            .items_center()
            .justify_center()
            .gap(surface::css(8.))
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::text())
                    .child("实时预览不可用"),
            )
            .child(
                div()
                    .text_size(surface::css(12.))
                    .line_height(surface::css(15.))
                    .text_color(Colors::placeholder())
                    .child("当前未接入摄像头流服务"),
            )
            .into_any_element()
    }
}

// Keep new source defaults when restoring an older local profile. An unrelated
// product's object or a changed JSON type cannot replace the current schema.
/// Port of the mounted pad's `uU`/`CU` helpers. `width`/`height` are the black
/// box content size; the rendered box adds its inline 1px border.
struct PanTiltBox {
    width: f32,
    height: f32,
    zoom: f32,
    max: f32,
}
impl PanTiltBox {
    /// White framing box origin inside the black box, in product pixels.
    fn white_origin(&self, pan: f32, tilt: f32) -> (f32, f32) {
        let outer_w = self.width + 2.;
        let outer_h = self.height + 2.;
        let white_w = self.width / self.zoom;
        let white_h = self.height / self.zoom;
        let mut left = (pan * ((outer_w - 1.) / 2. - white_w / 2.) / self.max - white_w / 2.
            + (outer_w - 1.) / 2.)
            .floor();
        if white_w + left > self.width {
            left = self.width - white_w;
        }
        let mut top = (-tilt * ((outer_h - 1.) / 2. - white_h / 2.) / self.max - white_h / 2.
            + (outer_h - 1.) / 2.)
            .floor();
        if top + white_h > self.height {
            top = (self.height - white_h).round();
        }
        (left.max(0.), top.max(0.))
    }
    /// `Tm`：白框左边距 → 平移值。界限是 `ceil(0.5 * max / -0.5) = -max`。
    fn pan_from_left(&self, left: f32, white_w: f32) -> f32 {
        let outer_w = self.width + 2.;
        let bound = (0.5 * self.max / -0.5).ceil();
        let mut pan = ((left + white_w / 2. - (outer_w - 1.) / 2.) * self.max
            / ((outer_w - 1.) / 2. - white_w / 2.))
            .ceil();
        if pan > -bound {
            pan = -bound;
        }
        if pan < bound {
            pan = bound;
        }
        pan
    }
    /// `Im`：白框上边距 → 倾斜值。
    fn tilt_from_top(&self, top: f32, white_h: f32) -> f32 {
        let outer_h = self.height + 2.;
        -((top + white_h / 2. - (outer_h - 1.) / 2.) * self.max
            / ((outer_h - 1.) / 2. - white_h / 2.))
            .ceil()
    }
}
/// Product input IDs and their source display names, module 6114.
const KEYS: &[(&str, &str, &str)] = include!("mapping_keys.rs");
fn is_modifier(id: &str) -> bool {
    KEYS.iter()
        .any(|(group, key, _)| *group == "modifiers" && *key == id)
}
fn input_label(id: &str) -> String {
    KEYS.iter()
        .find(|(_, key, _)| *key == id)
        .map_or_else(|| id.to_string(), |(_, _, label)| (*label).to_string())
}
/// Maps a keystroke key name onto the product's own input ID.
fn keystroke_input_id(key: &str) -> Option<&'static str> {
    let label = match key {
        "escape" => "Esc",
        "pageup" => "Page Up",
        "pagedown" => "Page Down",
        other => other,
    };
    KEYS.iter()
        .find(|(_, _, name)| name.eq_ignore_ascii_case(label))
        .map(|(_, id, _)| *id)
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
impl Render for SourceControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.spec.product_id == 691 && self.page == "OLED" {
            return self.render_oled_page(window, cx);
        }
        if self.spec.product_id == 179 && self.page == "TAB_CUSTOMIZE" {
            return self.render_receiver(window, cx);
        }
        if self.spec.layout.as_deref() == Some("camera") && self.page != "HELP" {
            if let Some(page) = self
                .spec
                .pages
                .iter()
                .find(|p| p.key == self.page && !p.sections.is_empty())
            {
                // All four current roots mount renderView() then their video
                // component inside `.advanced-camera-container`, which removes
                // the shared body padding. A video transport is not connected;
                // retain the empty video surface without fabricating a frame,
                // camera failure, or reconnect acknowledgement.
                return h_flex()
                    .size_full()
                    .min_h_0()
                    .min_w(surface::css(600.))
                    .items_stretch()
                    .overflow_hidden()
                    .font_family("Roboto")
                    .font_weight(FontWeight::NORMAL)
                    .text_size(surface::css(16.))
                    .text_color(gpui_kit::rgb(0xcccccc))
                    .child(self.render_camera_column(page, window, cx))
                    .child(
                        div()
                            .id("source-camera-video")
                            .relative()
                            .flex_1()
                            .h_full()
                            .bg(gpui_kit::rgb(0x000000)),
                    )
                    .into_any_element();
            }
        }
        let mut view = v_flex()
            .min_w(surface::css(600.))
            .w_full()
            .max_w(surface::css(1240.))
            .mx_auto()
            .p(surface::css(20.))
            .gap(surface::css(20.));
        // Accessory descriptors whose current renderer mounts the shared
        // product-image module declare this layout explicitly. Keep the same
        // 250px product-art contract; other descriptors retain their layouts.
        if self.spec.layout.as_deref() == Some("accessory") && self.page != "HELP" {
            view = view.child(surface::product_banner(self.spec.product_id, 0, 0, cx));
        }
        if self.page == "HELP" {
            if let Some(url) = &self.spec.support {
                let url = url.clone();
                view = view.child(
                    Button::new("source-product-support")
                        .label(crate::i18n::t("SUPPORT"))
                        .outline()
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                );
            }
        } else if let Some(page) = self
            .spec
            .pages
            .iter()
            .find(|p| p.key == self.page && !p.sections.is_empty())
        {
            if self.spec.layout.as_deref() == Some("legacy-camera") && self.page == "TAB_CUSTOMIZE"
            {
                // The shared Customize root mounts its camera preview before
                // the image and focus setting widgets.
                view = view.child(self.render_legacy_camera_preview());
            }
            let columns = page.sections.iter().any(|section| section.column.is_some());
            let mut left = v_flex()
                .flex_1()
                .min_w(surface::css(570.))
                .gap(surface::css(20.));
            let mut right = v_flex()
                .flex_1()
                .min_w(surface::css(570.))
                .gap(surface::css(20.));
            for section in &page.sections {
                let mut panel = surface::panel(crate::i18n::t(&section.title), cx);
                if let Some(description) = &section.description {
                    panel = panel.child(surface::note(crate::i18n::t(description), cx));
                }
                for control in &section.controls {
                    panel = panel.child(self.render_control(control, window, cx));
                }
                if let Some(note) = &section.note {
                    panel = panel.child(surface::note(crate::i18n::t(note), cx));
                }
                match section.column.as_deref() {
                    Some("left") => left = left.child(panel),
                    Some("right") => right = right.child(panel),
                    _ => view = view.child(panel),
                }
            }
            if columns {
                view = view.child(
                    h_flex()
                        .w_full()
                        .items_start()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .child(left)
                        .child(right),
                );
            }
        } else {
            view = view.child(surface::note("此页面的原生控件仍在接入。", cx));
        }
        view.into_any_element()
    }
}
