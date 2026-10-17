//! Native controller pages from each current product's mounted components.
//! `profile` and the independent Redux controller states remain separate.
//! Optimistic source state and actual service acknowledgements have separate owners.
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    Disableable, Selectable, StyledExt,
    button::Button,
    h_flex,
    slider::{SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::OnceLock};

use razer_i18n::t;
use razer_widgets::surface;
#[path = "gamepad_calibration.rs"]
mod calibration;
pub use calibration::state::{CalibrationIntent, CalibrationObservation};
#[path = "gamepad_lighting.rs"]
mod lighting;
#[path = "gamepad_lighting_runtime.rs"]
mod lighting_runtime;
#[path = "gamepad_lighting_state.rs"]
mod lighting_state;
#[path = "gamepad_quick_lighting_state.rs"]
mod quick_lighting_state;
#[path = "gamepad_quick_lighting.rs"]
mod quick_lighting;
#[path = "gamepad_chroma_runtime.rs"]
mod chroma_runtime;
pub use chroma_runtime::{ControllerChromaRequested, ControllerChromaCommand, ControllerChromaObservation, ControllerChromaProfile, ControllerSwitchOffLighting};
#[path = "gamepad_navigation.rs"]
mod navigation;
#[path = "gamepad_profile.rs"]
mod profile;
pub use lighting_runtime::ControllerLightingRequested;
#[path = "gamepad_trigger_calibration.rs"]
mod trigger_calibration;
#[path = "gamepad_trigger_calibration_state.rs"]
mod trigger_calibration_state;
#[path = "gamepad_trigger_runtime.rs"]
mod trigger_runtime;
#[path = "gamepad_trigger_visualization.rs"]
mod trigger_visualization;
#[path = "gamepad_triggers.rs"]
mod triggers;
pub use navigation::TriggerNavigation;
pub use trigger_calibration_state::{TriggerCalibrationIntent, TriggerCalibrationObservation};
pub use trigger_runtime::ControllerTriggerRequested;
#[path = "gamepad_calibration_selection.rs"]
mod calibration_selection;
#[path = "gamepad_customize_artwork.rs"]
mod customize_artwork;

#[path = "gamepad_deadzone_dialog.rs"]
mod deadzone_dialog;
#[path = "kitsune.rs"]
mod kitsune;
#[cfg(test)]
#[path = "../../../testdata/resource_cases.rs"]
mod resource_fixtures;
#[path = "gamepad_thumbsticks.rs"]
mod thumbsticks;
#[path = "gamepad_trigger_calibration_renderer.rs"]
mod trigger_calibration_renderer;

#[derive(Deserialize)]
pub struct GamepadProductSpec {
    product_id: u32,
    profile: Value,
    controller: Value,
    pages: Vec<String>,
    button_controls: Vec<Value>,
    assignments: Vec<String>,
    arcade: bool,
    socd_modes: BTreeMap<String, String>,
    trigger_reset: Value,
    analog_mode: i64,
    digital_mode: i64,
    standard_mode: i64,
    circular_mode: i64,
    deadzone_steps: Vec<i64>,
    sensitivity_steps: Vec<i64>,
    power_minutes: Vec<i64>,
    brightness_step: Option<f32>,
    controller_lighting: bool,
    effects: Vec<Value>,
    polling_rates: Vec<i64>,
    info: Value,
}

pub fn source_product(pid: u32) -> Option<&'static GamepadProductSpec> {
    static PRODUCTS: super::source_catalog::SourceCatalog<GamepadProductSpec> =
        super::source_catalog::SourceCatalog::new(|| read_gamepad_products_catalog());
    PRODUCTS.get(pid)
}

pub struct GamepadProductChanged;
pub struct GamepadNavigationRequested {
    page_key: &'static str,
}
impl GamepadNavigationRequested {
    pub fn page_key(&self) -> &'static str {
        self.page_key
    }
}

/// 源码 `GR` 的功耗取值标签：`item.value >= 60 ? ra.pHP : ra.yvH`，同时
/// `data.value` 在 `>= 60` 时除以 60。两个别名经导出表解析为 `MIN`
/// （"{{value}} min."）与 `SEC`（"{{value}} sec."），说明该组件按**秒**判断：
/// 不足 60 显示秒、满 60 起按分钟显示。
pub fn power_saving_label(value: i64) -> String {
    if value >= 60 {
        razer_i18n::t_value("MIN", value / 60)
    } else {
        razer_i18n::t_value("SEC", value)
    }
}

/// `JP` 按扳机侧选的三对键：`we.LST`/`we.gzX`（标题）、`we.Vwf`/`we.ukk`
/// （模拟分支的 `.h1-body`）、`we.C7E`/`we.Fh7`（数字分支的 `.h1-body`）。
pub fn trigger_keys(prefix: &str) -> (&'static str, &'static str, &'static str) {
    if prefix == "LEFT" {
        (
            "LEFT_TRIGGER_MODE",
            "LEFT_TRIGGER_RANGE",
            "LEFT_ACTUATION_POINT",
        )
    } else {
        (
            "RIGHT_TRIGGER_MODE",
            "RIGHT_TRIGGER_RANGE",
            "RIGHT_ACTUATION_POINT",
        )
    }
}

/// `QP` 手柄的越界规则：起点最多到终点 −1，终点最少到起点 +1（原版
/// `Math.min(value, max-1)` / `Math.max(value, min+1)`）。
pub fn range_handle_value(handle: RangeHandle, value: i64, start: i64, end: i64) -> i64 {
    match handle {
        RangeHandle::Start => value.min(end - 1).clamp(0, 99),
        RangeHandle::End => value.max(start + 1).clamp(1, 100),
    }
}

#[cfg(test)]
#[path = "gamepad_products_tests.rs"]
mod tests;

/// `QP` 的两个手柄。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangeHandle {
    Start,
    End,
}

/// `QP` 的几何与临时取值。canvas 记录容器 bounds，鼠标位移按原生 range
/// thumb 的可移动宽度换算；预览不进入本地 profile 快照。
#[derive(Clone, Default)]
struct RangeDrag {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    pointer: Option<RangePointer>,
    preview: Option<(i64, i64)>,
}

#[derive(Clone, Copy)]
struct RangePointer {
    handle: RangeHandle,
    origin: Pixels,
    value: i64,
    travel: Pixels,
}

pub struct GamepadProductWorkspace {
    layout_id: u32,
    edition_id: u32,
    spec: &'static GamepadProductSpec,
    range_drags: BTreeMap<String, RangeDrag>,
    page: String,
    draft: Value,
    selected_button: Option<String>,
    customize_hover: Option<String>,
    sensitivity: bool,
    thumbstick_ui: thumbsticks::ThumbstickUi,
    low_deadzone: Option<(String, Value)>,
    previous_deadzones: BTreeMap<String, Value>,
    deadzone_dialog: Option<deadzone_dialog::DialogState>,
    calibration_state: calibration::state::CalibrationState,
    calibration_resources: crate::runtime_resources::TextResources,
    trigger_calibration_resources: trigger_calibration_renderer::Resources,
    calibration_dialog: Option<calibration::CalibrationDialog>,
    calibration_popup: Option<calibration::CalibrationDialog>,
    trigger_calibration_state: trigger_calibration_state::TriggerCalibrationState,
    trigger_calibration_popup: Option<trigger_calibration::TriggerDialog>,
    trigger_frame_pending: bool,
    trigger_runtime: trigger_runtime::TriggerRuntime,
    lighting_runtime: lighting_runtime::LightingRuntime,
    lighting_ui: lighting::LightingUi,
    quick_lighting_ui: Option<quick_lighting::QuickLightingUi>,
    chroma_runtime: chroma_runtime::ChromaRuntime,
    trigger_visualization: trigger_visualization::TriggerVisualization,
    controller_input_sample: Option<razer_device::controller_input::Sample>,
    controller_notifications: std::collections::VecDeque<razer_device::controller_input::Event>,
    calibration_selection_hover: Option<u8>,
    calibration_bounds: Rc<Cell<Bounds<Pixels>>>,
    thumbstick_bounds: Rc<Cell<Bounds<Pixels>>>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<GamepadProductChanged> for GamepadProductWorkspace {}
impl EventEmitter<GamepadNavigationRequested> for GamepadProductWorkspace {}
impl EventEmitter<CalibrationIntent> for GamepadProductWorkspace {}
impl EventEmitter<TriggerCalibrationIntent> for GamepadProductWorkspace {}
impl EventEmitter<ControllerTriggerRequested> for GamepadProductWorkspace {}

impl GamepadProductWorkspace {
    pub fn new(pid: u32, layout_id: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited gamepad product");
        let mut this = Self {
            layout_id,
            edition_id: 0,
            spec,
            range_drags: BTreeMap::new(),
            page: "TAB_CUSTOMIZE".into(),
            draft: json!({"profile": spec.profile, "controller": spec.controller}),
            selected_button: None,
            customize_hover: None,
            sensitivity: false,
            thumbstick_ui: Default::default(),
            low_deadzone: None,
            previous_deadzones: BTreeMap::new(),
            deadzone_dialog: None,
            calibration_state: Default::default(),
            calibration_resources: Default::default(),
            trigger_calibration_resources: Default::default(),
            calibration_dialog: None,
            calibration_popup: None,
            trigger_calibration_state: Default::default(),
            trigger_calibration_popup: None,
            trigger_frame_pending: false,
            trigger_runtime: Default::default(),
            lighting_runtime: lighting_runtime::LightingRuntime::new(pid),
            lighting_ui: lighting::LightingUi::new(window, cx),
            quick_lighting_ui: None,
            chroma_runtime: Default::default(),
            trigger_visualization: Default::default(),
            controller_input_sample: None,
            controller_notifications: Default::default(),
            calibration_selection_hover: None,
            calibration_bounds: Rc::new(Cell::new(Bounds::default())),
            thumbstick_bounds: Rc::new(Cell::new(Bounds::default())),
            sliders: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
        };
        if let Err(error) = this.restore_lighting_profile() {
            eprintln!("[controller-lighting] Invalid initial profile: {error:#}");
        }
        this.initialize_quick_lighting_controls(window, cx);
        if spec.pages.iter().any(|p| p == "TRIGGERS") {
            for side in ["leftTrigger", "rightTrigger"] {
                this.range_drags
                    .insert(format!("/controller/{side}"), RangeDrag::default());
                this.add_slider(
                    &format!("/controller/{side}/actuationPoint"),
                    1.,
                    100.,
                    1.,
                    window,
                    cx,
                );
            }
        }
        if let Some(step) = spec.brightness_step {
            this.add_slider("/profile/brightness/value", 0., 100., step, window, cx);
            if !spec.controller_lighting {
                this.add_slider(
                    "/profile/switchOffLighting/idleMinutes",
                    1.,
                    15.,
                    1.,
                    window,
                    cx,
                );
            }
        }
        this.initialize_thumbstick_controls(window, cx);
        this.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_range_edits(cx);
                    this.cancel_thumbstick_edits(cx);
                }
            }));
        this.refresh_trigger_visualization();
        this.subscriptions
            .push(razer_assets::observe_resources(cx, |this, _, cx| {
                let mut changed = this.calibration_resources.refresh();
                changed |= this.trigger_calibration_resources.refresh();
                if this.page == "THUMBSTICKS" && this.thumbstick_ui.artwork_pending() {
                    this.refresh_thumbstick_artwork();
                    changed = true;
                }
                if this.trigger_artwork_pending() {
                    this.refresh_trigger_visualization();
                    changed = true;
                }
                if changed {
                    cx.notify();
                }
            }));
        this
    }

    pub fn set_page(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            if self.page == "TAB_LIGHTING" {
                self.lighting_ui.cancel(cx);
                self.dismiss_quick_lighting_controls(cx);
            }
            self.customize_hover = None;
            self.cancel_range_edits(cx);
            self.cancel_thumbstick_edits(cx);
            if self.page == "TAB_CALIBRATION"
                || self.calibration_popup.is_some()
                || self.trigger_calibration_popup.is_some()
            {
                self.leave_calibration(window, cx);
            }
            // 2636's mounted thumbstick component initializes prevLeft/Right
            // once on entry. Continue does not replace these rollback values.
            if key == "THUMBSTICKS" {
                self.sensitivity = false;
                self.previous_deadzones.clear();
                for side in ["leftStick", "rightStick"] {
                    let path = format!("/controller/{side}/deadzoneValue");
                    if let Some(value) = self.draft.pointer(&path) {
                        self.previous_deadzones.insert(path, value.clone());
                    }
                }
            }
            self.page = key.into();
            if key == "TAB_CALIBRATION" {
                self.request_calibration_resources();
            }
            self.refresh_thumbstick_artwork();
            self.refresh_trigger_visualization();
            self.dismiss_deadzone(false, window, cx);
            self.low_deadzone = None;
            self.request_trigger_read(cx);
            if key == "TAB_LIGHTING" {self.refresh_controller_chroma(cx);}
            cx.notify();
        }
    }

    pub fn snapshot(&self) -> Value {
        self.draft.clone()
    }

    pub fn restore(&mut self, saved: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.trigger_runtime.invalidate();
        self.lighting_runtime.invalidate();
        self.chroma_runtime.invalidate();
        self.lighting_ui.cancel(cx);
        self.dismiss_quick_lighting_controls(cx);
        self.cancel_range_edits(cx);
        self.cancel_thumbstick_edits(cx);
        self.leave_calibration(window, cx);
        self.draft = json!({"profile": self.spec.profile, "controller": self.spec.controller});
        if let Some(saved) =
            saved.filter(|v| v["profile"].is_object() && v["controller"].is_object())
        {
            merge_known(&mut self.draft, saved);
            if let Err(error) =
                profile::restore_features(&mut self.draft["profile"], &saved["profile"], self.spec)
            {
                eprintln!(
                    "[controller-profile] Saved feature records could not be restored: {error:#}"
                );
            }
        }
        self.selected_button = None;
        self.customize_hover = None;
        self.dismiss_deadzone(false, window, cx);
        self.low_deadzone = None;
        if let Err(error) = self.restore_lighting_profile() {
            eprintln!("[controller-lighting] Saved settings could not be restored: {error:#}");
        }
        self.sync_sliders(window, cx);
        self.sync_quick_lighting_controls(window,cx);
        self.refresh_trigger_visualization();
        self.refresh_thumbstick_artwork();
        self.request_trigger_read(cx);
        self.request_controller_lighting(cx);
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
                    let value = value.start().clamp(min, max).round() as i64;
                    this.write(&key, json!(value), cx);
                    this.sync_sliders(window, cx);
                }
            },
        ));
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
        self.sync_thumbstick_controls(window, cx);
    }

    fn write(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        if let Some((root, field)) = path.rsplit_once('/') {
            if matches!(field, "mode" | "startRange" | "endRange") {
                self.cancel_range_edit(root, cx);
            }
        }
        if let Some(target) = self.draft.pointer_mut(path) {
            if *target != value {
                *target = value;
                if path.starts_with("/profile/brightness/") {
                    self.controller_brightness_changed(path, cx);
                }
                if self.spec.product_id==4115 && path.starts_with("/profile/switchOffLighting/") {self.controller_switch_off_changed(cx);}
                if let Some((root, _)) = path.rsplit_once('/') {
                    if matches!(root, "/controller/leftTrigger" | "/controller/rightTrigger") {
                        self.trigger_changed(root, cx);
                    } else if matches!(
                        root,
                        "/controller/leftStick"
                            | "/controller/rightStick"
                            | "/controller/circularity"
                    ) {
                        self.thumbstick_changed(path, cx);
                        if path.ends_with("/deadzoneValue") {
                            self.refresh_thumbstick_artwork();
                        }
                    }
                }
                cx.emit(GamepadProductChanged);
                cx.notify();
            }
        }
    }

    fn number(&self, path: &str) -> i64 {
        self.draft
            .pointer(path)
            .and_then(Value::as_i64)
            .expect("audited numeric controller field")
    }

    fn checked(&self, path: &str) -> bool {
        self.draft
            .pointer(path)
            .and_then(Value::as_bool)
            .expect("audited boolean controller field")
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
        Button::new(SharedString::from(format!("gamepad-{path}-{value}")))
            .label(label)
            .outline()
            .disabled(!enabled)
            .selected(self.draft.pointer(&path) == Some(&value))
            .on_click(cx.listener(move |this, _, _, cx| {
                if enabled {
                    this.write(&path, value.clone(), cx);
                }
            }))
    }

    /// `QP`：`.rangeSlider` 双柄滑条。两条 6px、圆角 5px 的条
    /// （`.rangeSliderBackground{background:#44d62c;opacity:.3}` 与
    /// `.rangeSliderHighlight{background:#44d62c;left:start%;right:(100-end)%}`，
    /// 都在 `top:18px`），两个 20px 绿色圆柄（悬停 `#5d5d5d` + `2px #44d62c`、
    /// 按下 `#383838` + `2px #44d62c`），柄上方 `.sliderTipBar` 的绿色数值气泡
    /// （`#000`、12px、`padding:4px 9px`、`top:-16px`、`translateX(-50%)`），
    /// 以及下方两端对齐的 `0`/`100`。手柄不能互相越过，对应原版的
    /// `Math.min(value, max-1)` / `Math.max(value, min+1)`。
    fn range_control(&self, root: &str, start: i64, end: i64, cx: &Context<Self>) -> AnyElement {
        let Some(drag) = self.range_drags.get(root) else {
            return div().into_any_element();
        };
        let (start, end) = drag.preview.unwrap_or((start, end));
        let bounds_cell = drag.bounds.clone();
        // 原版用 `:active` 表示手柄被按住；本地按拖拽状态给同样的
        // `background:#383838;border:2px solid #44d62c`。
        let thumb = |value: i64, handle: RangeHandle| {
            let pressed = drag.pointer.is_some_and(|pointer| pointer.handle == handle);
            let root_down = root.to_owned();
            let root_key = root.to_owned();
            let root_up = root.to_owned();
            let root_up_out = root.to_owned();
            let prefix = if root.ends_with("leftTrigger") {
                "LEFT"
            } else {
                "RIGHT"
            };
            let (_, range_key, _) = trigger_keys(prefix);
            div()
                .absolute()
                .left(relative(value as f32 / 100.))
                // Native range thumb centers travel over input width minus 20px.
                // The input's -10px margin cancels the initial half-thumb inset.
                .ml(surface::css(-20. * value as f32 / 100.))
                .top(surface::css(11.))
                .w_0()
                .flex()
                .justify_center()
                .child(
                    BaseButton::new(SharedString::from(format!(
                        "gamepad-range-{root}-{handle:?}"
                    )))
                    .role(Role::Slider)
                    .accessibility_label(t(range_key))
                    .aria_numeric_value(value as f64)
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(100.)
                    .aria_numeric_value_step(1.)
                    .flex_shrink_0()
                    .w(surface::css(20.))
                    .h(surface::css(20.))
                    .rounded_full()
                    .bg(if pressed { rgb(0x383838) } else { rgb(0x44d62c) })
                    .when(pressed, |thumb| {
                        thumb.border_2().border_color(rgb(0x44d62c))
                    })
                    .when(!pressed, |thumb| {
                        thumb.hover(|style| {
                            style
                                .bg(rgb(0x5d5d5d))
                                .border_2()
                                .border_color(rgb(0x44d62c))
                        })
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.range_drag_start(event, &root_down, handle, window, cx);
                        }),
                    )
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        this.range_key_edit(&root_key, handle, event, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                            this.finish_range_edit(&root_up, event.position.x, cx);
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                            this.finish_range_edit(&root_up_out, event.position.x, cx);
                        }),
                    ),
                )
        };
        let tip = |value: i64| {
            div()
                .absolute()
                .left(relative(value as f32 / 100.))
                .top(surface::css(-16.))
                .w_0()
                .flex()
                .justify_center()
                .child(range_tip_label(value))
        };
        let root_events = root.to_owned();
        let owner = cx.weak_entity();
        let dragging = drag.pointer.is_some();
        v_flex()
            .mt(surface::css(20.))
            .child(
                div()
                    .relative()
                    .ml(surface::css(10.))
                    .mr(surface::css(-12.))
                    .mb(surface::css(-8.))
                    .h(surface::css(40.))
                    .child(
                        canvas(
                            move |bounds, _, _| bounds_cell.set(bounds),
                            move |_, _, window, _| {
                                if !dragging {
                                    return;
                                }
                                let moving = owner.clone();
                                let moving_root = root_events.clone();
                                window.on_mouse_event(
                                    move |event: &MouseMoveEvent, phase, _, cx| {
                                        if phase == DispatchPhase::Bubble {
                                            let _ = moving.update(cx, |this, cx| {
                                                this.range_drag_move(event, &moving_root, cx);
                                            });
                                        }
                                    },
                                );
                                let release = owner.clone();
                                let release_root = root_events.clone();
                                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                                    if phase == DispatchPhase::Bubble
                                        && event.button == MouseButton::Left
                                    {
                                        let _ = release.update(cx, |this, cx| {
                                            this.finish_range_edit(
                                                &release_root,
                                                event.position.x,
                                                cx,
                                            );
                                        });
                                    }
                                });
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(-10.))
                            .right(surface::css(12.))
                            .top(surface::css(18.))
                            .h(surface::css(6.))
                            .rounded(surface::css(5.))
                            .bg(rgba(0x44d62c4d)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(relative(start as f32 / 100.))
                            .right(relative((100 - end) as f32 / 100.))
                            .ml(surface::css(-10.))
                            .mr(surface::css(12.))
                            .top(surface::css(18.))
                            .h(surface::css(6.))
                            .rounded(surface::css(5.))
                            .bg(rgb(0x44d62c)),
                    )
                    .child(
                        div()
                            .relative()
                            .w(relative(0.96))
                            .child(tip(start))
                            .child(tip(end)),
                    )
                    .child(thumb(start, RangeHandle::Start))
                    .child(thumb(end, RangeHandle::End)),
            )
            .child(h_flex().justify_between().child("0").child("100"))
            .into_any_element()
    }

    fn range_pointer_value(&self, root: &str, x: Pixels) -> Option<i64> {
        let pointer = self.range_drags.get(root)?.pointer?;
        if pointer.travel <= px(0.) {
            return None;
        }
        let delta = (x - pointer.origin) / pointer.travel;
        Some(
            (pointer.value as f32 + delta * 100.)
                .round()
                .clamp(0., 100.) as i64,
        )
    }

    /// Current CSS disables hit testing on the input track and enables it only
    /// on the two thumbs. Preserve the grab offset instead of jumping on press.
    fn range_drag_start(
        &mut self,
        event: &MouseDownEvent,
        root: &str,
        handle: RangeHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != "TRIGGERS" || self.number(&format!("{root}/mode")) != self.spec.analog_mode
        {
            return;
        }
        let Some(drag) = self.range_drags.get(root) else {
            return;
        };
        let travel = drag.bounds.get().size.width - surface::css(20.).to_pixels(window.rem_size());
        if travel <= px(0.) {
            return;
        }
        let (start, end) = drag.preview.unwrap_or_else(|| {
            (
                self.number(&format!("{root}/startRange")),
                self.number(&format!("{root}/endRange")),
            )
        });
        self.cancel_range_edits(cx);
        let Some(drag) = self.range_drags.get_mut(root) else {
            return;
        };
        drag.preview = Some((start, end));
        drag.pointer = Some(RangePointer {
            handle,
            origin: event.position.x,
            value: if handle == RangeHandle::Start { start } else { end },
            travel,
        });
        cx.notify();
    }

    fn range_drag_move(&mut self, event: &MouseMoveEvent, root: &str, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.cancel_range_edits(cx);
            return;
        }
        let Some(value) = self.range_pointer_value(root, event.position.x) else {
            return;
        };
        self.range_drag_apply(root, value, cx);
    }

    fn range_drag_apply(&mut self, root: &str, value: i64, cx: &mut Context<Self>) {
        let Some(drag) = self.range_drags.get_mut(root) else {
            return;
        };
        let (Some(pointer), Some((start, end))) = (drag.pointer, drag.preview) else {
            return;
        };
        let handle = pointer.handle;
        let next = range_handle_value(handle, value, start, end);
        match handle {
            RangeHandle::Start => drag.preview = Some((next, end)),
            RangeHandle::End => drag.preview = Some((start, next)),
        }
        cx.notify();
    }

    fn range_key_edit(
        &mut self,
        root: &str,
        handle: RangeHandle,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        if self.page != "TRIGGERS" || self.number(&format!("{root}/mode")) != self.spec.analog_mode
        {
            return;
        }
        if event.keystroke.key == "escape" {
            self.cancel_range_edit(root, cx);
            cx.stop_propagation();
            return;
        }
        let Some(drag) = self.range_drags.get(root) else {
            return;
        };
        if drag.pointer.is_some() {
            return;
        }
        let (start, end) = drag.preview.unwrap_or_else(|| {
            (
                self.number(&format!("{root}/startRange")),
                self.number(&format!("{root}/endRange")),
            )
        });
        let current = if handle == RangeHandle::Start { start } else { end };
        let requested = match event.keystroke.key.as_str() {
            "left" | "down" => current - 1,
            "right" | "up" => current + 1,
            "home" => 0,
            "end" => 100,
            _ => return,
        };
        let next = range_handle_value(handle, requested, start, end);
        if let Some(drag) = self.range_drags.get_mut(root) {
            // The source input's onChange updates component state. Its
            // changeValue callback is only called by onMouseUp.
            drag.preview = Some(if handle == RangeHandle::Start {
                (next, end)
            } else {
                (start, next)
            });
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn finish_range_edit(&mut self, root: &str, x: Pixels, cx: &mut Context<Self>) {
        if self.page != "TRIGGERS" || self.number(&format!("{root}/mode")) != self.spec.analog_mode
        {
            self.cancel_range_edits(cx);
            return;
        }
        if let Some(value) = self.range_pointer_value(root, x) {
            self.range_drag_apply(root, value, cx);
        }
        let Some(drag) = self.range_drags.get_mut(root) else {
            return;
        };
        if drag.pointer.take().is_none() {
            return;
        }
        let Some((start, end)) = drag.preview.take() else {
            return;
        };
        // Current per-trigger range component commits both fields on mouseup.
        // Pointer preview never enters a profile snapshot or emits Changed.
        let mut changed = false;
        for (field, value) in [("startRange", start), ("endRange", end)] {
            if let Some(target) = self.draft.pointer_mut(&format!("{root}/{field}")) {
                if *target != json!(value) {
                    *target = json!(value);
                    changed = true;
                }
            }
        }
        if changed {
            self.trigger_changed(root, cx);
            cx.emit(GamepadProductChanged);
        }
        cx.notify();
    }

    pub fn cancel_range_edits(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for drag in self.range_drags.values_mut() {
            changed |= drag.pointer.take().is_some() | drag.preview.take().is_some();
        }
        if changed {
            cx.notify();
        }
    }

    fn cancel_range_edit(&mut self, root: &str, cx: &mut Context<Self>) {
        if let Some(drag) = self.range_drags.get_mut(root) {
            if drag.pointer.take().is_some() | drag.preview.take().is_some() {
                cx.notify();
            }
        }
    }

    fn reset_trigger(&mut self, root: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_range_edit(root, cx);
        let fields: &[&str] = if self.number(&format!("{root}/mode")) == self.spec.analog_mode {
            &["startRange", "endRange"]
        } else {
            &["actuationPoint", "isRapidTrigger"]
        };
        let mut changed = false;
        for field in fields {
            if let (Some(target), Some(value)) = (
                self.draft.pointer_mut(&format!("{root}/{field}")),
                self.spec.trigger_reset.get(*field),
            ) {
                if *target != *value {
                    *target = value.clone();
                    changed = true;
                }
            }
        }
        self.sync_sliders(window, cx);
        if changed {
            self.trigger_changed(root, cx);
            cx.emit(GamepadProductChanged);
            cx.notify();
        }
    }

    fn set_mapping(&mut self, input: &str, assignment: Option<&str>, cx: &mut Context<Self>) {
        let Some(button) = self
            .spec
            .button_controls
            .iter()
            .find(|b| b["inputID"].as_str() == Some(input))
        else {
            return;
        };
        let Some(mappings) = self
            .draft
            .pointer_mut("/profile/mappings")
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        mappings.retain(|m| m["inputID"].as_str() != Some(input));
        if let Some(assignment) = assignment {
            let mut mapping = json!({"inputID":input,"inputType":button["inputType"],"controllerInput":input,"isHyperShift":false});
            if assignment == "DISABLE" {
                mapping["outputType"] = json!("disableGroup");
                mapping["disableGroup"] = json!({});
            } else {
                mapping["outputType"] = json!("controllerGroup");
                mapping["controllerGroup"] = json!({"controllerAssignment":assignment});
            }
            mappings.push(mapping);
        }
        cx.emit(GamepadProductChanged);
        cx.notify();
    }

    /// The `displayMode=armory` root mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.customize(cx)
    }

    fn customize(&self, cx: &Context<Self>) -> AnyElement {
        if self.spec.product_id == 4115 {
            return self.kitsune_customize(cx);
        }
        let mut left = surface::panel(t("TAB_CUSTOMIZE"), cx);
        if !self.spec.arcade {
            if let Some(input) = &self.selected_button {
                if let Some(button) = self
                    .spec
                    .button_controls
                    .iter()
                    .find(|b| b["inputID"].as_str() == Some(input))
                {
                    let supported = |key: &str| {
                        button["functionList"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(key)))
                    };
                    let current = self.draft["profile"]["mappings"]
                        .as_array()
                        .and_then(|a| a.iter().find(|m| m["inputID"].as_str() == Some(input)));
                    let selected =
                        current.and_then(|m| m["controllerGroup"]["controllerAssignment"].as_str());
                    left = left.child(div().font_bold().child(input.clone()));
                    if supported("DEFAULT") {
                        let input = input.clone();
                        left = left.child(
                            Button::new("gamepad-mapping-default")
                                .label(t("DEFAULT"))
                                .outline()
                                .selected(current.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_mapping(&input, None, cx)
                                })),
                        );
                    }
                    if supported("CONTROLLER_V2") || supported("CONTROLLER_PLAYSTATION") {
                        left =
                            left.child(h_flex().gap_2().flex_wrap().children(
                                self.spec.assignments.iter().map(|assignment| {
                                    let input = input.clone();
                                    let assignment = assignment.clone();
                                    Button::new(SharedString::from(format!(
                                        "gamepad-assign-{assignment}"
                                    )))
                                    .label(t(&assignment))
                                    .outline()
                                    .selected(selected == Some(assignment.as_str()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_mapping(&input, Some(&assignment), cx)
                                    }))
                                }),
                            ));
                    }
                    if supported("DISABLE") {
                        let input = input.clone();
                        left = left.child(
                            Button::new("gamepad-mapping-disable")
                                .label(t("DISABLE"))
                                .outline()
                                .selected(
                                    current.is_some_and(|m| m["outputType"] == "disableGroup"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_mapping(&input, Some("DISABLE"), cx)
                                })),
                        );
                    }
                }
            }
        }
        let mut right = v_flex().gap_5().child(
            surface::panel(t("POLLING_RATE"), cx).child(h_flex().gap_2().flex_wrap().children(
                self.spec.polling_rates.iter().map(|rate| {
                    self.choice(
                        "/profile/pollingRate",
                        json!(rate),
                        format!("{rate} Hz"),
                        true,
                        cx,
                    )
                }),
            )),
        );
        if self.spec.arcade {
            right = right.child(
                surface::panel(t("MODE_SWITCHER"), cx)
                    .child(self.choice(
                        "/controller/modeSwitcher/mode",
                        json!("safe"),
                        t("MODE_SWITCHER_SAFE"),
                        true,
                        cx,
                    ))
                    .child(surface::note(t("MODE_SWITCHER_SAFE_DESC"), cx))
                    .child(self.choice(
                        "/controller/modeSwitcher/mode",
                        json!("standard"),
                        t("MODE_SWITCHER_STANDARD"),
                        true,
                        cx,
                    ))
                    .child(surface::note(t("MODE_SWITCHER_STANDARD_DESC"), cx)),
            );
            let mut socd = surface::panel(t("DPAD_SOCD_SETTINGS"), cx)
                .child(surface::note(t("DPAD_SOCD_SETTINGS_DESC"), cx));
            for (key, label) in [
                ("NEUTRAL", "NEUTRAL_SOCD"),
                ("FIRST", "FIRST_INPUT_SOCD"),
                ("LAST", "LAST_INPUT_SOCD"),
                ("ABSOLUTE_UP", "ABSOLUTE_UP_SOCD"),
                ("NONE", "NO_SOCD"),
            ] {
                if let Some(value) = self.spec.socd_modes.get(key) {
                    socd = socd.child(self.choice(
                        "/controller/dpad/socdSettings/value",
                        json!(value),
                        t(label),
                        true,
                        cx,
                    ));
                }
            }
            right = right.child(socd);
        }
        v_flex()
            .w_full()
            .child(self.customize_artwork(cx))
            .child(
                surface::page_columns()
                    .when(self.selected_button.is_some(), |columns| {
                        columns.child(surface::page_column(left))
                    })
                    .child(surface::page_column(right)),
            )
            .into_any_element()
    }

    fn clutch_assigned(&self, side: &str) -> bool {
        self.draft["profile"]["mappings"]
            .as_array()
            .is_some_and(|mappings| {
                mappings.iter().any(|m| {
                    let assignment = m["controllerGroup"]["controllerAssignment"].as_str();
                    assignment == Some("GLOBAL_SENSITIVITY_CLUTCH")
                        || assignment
                            == Some(if side == "leftStick" {
                                "LEFT_SENSITIVITY_CLUTCH"
                            } else {
                                "RIGHT_SENSITIVITY_CLUTCH"
                            })
                })
            })
    }

    /// The `displayMode=chromaApp` popup mounts this page without the product
    /// chrome; the renderer itself is shared, so no separate layout is faked.
    pub fn lighting_element(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.lighting(window, cx)
    }

    /// 源码灯光页：左侧亮度组件（标题 `BRIGHTNESS_HEADER`、标题行开关、
    /// 右上帮助 `BRIGHTNESS_TOOLTIP`、0–100 滑条）加上「关闭灯光」组件
    /// （`SWITCH_OFF_LIGHTING_HEADER` + `SWITCH_OFF_LIGHTING_TOOLTIP`，两个
    /// `.check-item` 与 1–15 滑条），右侧快速效果。
    fn lighting(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.spec.product_id==4115 {return self.kitsune_lighting(window,cx);}
        if self.spec.controller_lighting {
            return surface::page_columns()
                .child(surface::page_column(
                    self.controller_brightness_panel(window, cx),
                ))
                .child(surface::page_column(self.controller_preset_panel(cx)))
                .into_any_element();
        }
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .child(self.controller_brightness_panel(window, cx))
                    .when(!self.spec.controller_lighting, |column| {
                        column.child(self.switch_off_lighting(window, cx))
                    }),
            ))
            .child(surface::page_column({
                let path = "/profile/quickEffects/selectedEffectId";
                surface::panel(t("EFFECTS"), cx).children(self.spec.effects.iter().filter_map(
                    |effect| {
                        Some(self.choice(
                            path,
                            effect["id"].clone(),
                            t(effect["name"].as_str()?),
                            true,
                            cx,
                        ))
                    },
                ))
            }))
            .into_any_element()
    }

    /// 与键盘/鼠标同源的「关闭灯光」组件：面板标题 `SWITCH_OFF_LIGHTING_HEADER`、
    /// 右上帮助 `SWITCH_OFF_LIGHTING_TOOLTIP`，两个勾选项用 `DISPLAY_TURNED_OFF`
    /// 与 `IDLE_FOR_MIN`（`extraClass:"has-slider"` → 滑条 `margin-left:30px;
    /// width:490px`），滑条 1–15、灰标 `1`/`15`。
    fn switch_off_lighting(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let brightness_on = self.checked("/profile/brightness/isEnabled");
        let display_on = self.checked("/profile/switchOffLighting/isDisplayOn");
        let idle_on = self.checked("/profile/switchOffLighting/isIdleEnabled");
        let widget = surface::panel_with_help(
            t("SWITCH_OFF_LIGHTING_HEADER"),
            surface::help_control(
                "gamepad-switch-off-lighting-help",
                t("SWITCH_OFF_LIGHTING_TOOLTIP"),
            ),
            cx,
        )
        .child(
            surface::check_item(
                "gamepad-switch-off-display",
                t("DISPLAY_TURNED_OFF"),
                display_on,
                !brightness_on,
                window,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.write(
                    "/profile/switchOffLighting/isDisplayOn",
                    json!(!display_on),
                    cx,
                )
            })),
        )
        .child(
            surface::check_item(
                "gamepad-switch-off-idle",
                t("IDLE_FOR_MIN"),
                idle_on,
                !brightness_on,
                window,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.write(
                    "/profile/switchOffLighting/isIdleEnabled",
                    json!(!idle_on),
                    cx,
                )
            })),
        )
        .child({
            let mut column = v_flex().ml(surface::css(30.)).w(surface::css(490.));
            if let Some(slider) = self.sliders.get("/profile/switchOffLighting/idleMinutes") {
                let value=self.number("/profile/switchOffLighting/idleMinutes") as f32;
                column = column.child(razer_widgets::source_slider::SourceSlider::new(slider,(value-1.)/14.).enabled(brightness_on&&idle_on).tip(Some(value.to_string())).range_labels("1","15",razer_widgets::theme::GamepadTriggerColors::text()));
            }
            column
        });
        widget.into_any_element()
    }

    fn power(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_help(
            t("POWER_SAVING_HEADER"),
            surface::help_control("gamepad-power-saving-help", t("POWER_SAVING_TOOLTIP")),
            cx,
        )
        .child(surface::note(t("CONTROLLER_POWER_SAVING_DESC"), cx))
        .child(
            h_flex()
                .gap_2()
                .children(self.spec.power_minutes.iter().map(|value| {
                    self.choice(
                        "/controller/power/powerSaving/value",
                        json!(value),
                        power_saving_label(*value),
                        self.checked("/controller/power/powerSaving/isEnabled"),
                        cx,
                    )
                })),
        )
        .into_any_element()
    }

    fn calibration(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.has_popup_calibration() {
            return calibration_selection::render(
                self.spec.product_id,
                self.edition_id,
                self.calibration_selection_hover,
                cx,
            );
        }
        if self.has_page_calibration() {
            return self.render_calibration(window, cx);
        }
        // No mounted Calibration caller exists for the remaining products.
        div().into_any_element()
    }
}

impl Render for GamepadProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule_trigger_frame(window, cx);
        let content = match self.page.as_str() {
            "TAB_CUSTOMIZE" => self.customize(cx),
            "TRIGGERS" => self.triggers(window, cx),
            "THUMBSTICKS" => self.thumbsticks(window, cx),
            "TAB_LIGHTING" => self.lighting(window, cx),
            "TAB_POWER" => self.power(cx),
            "TAB_CALIBRATION" => self.calibration(window, cx),
            _ => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        };
        super::product_surface::body()
            .child(content)
            .when(self.deadzone_dialog.is_some(), |body| {
                body.child(self.render_deadzone_dialog(window, cx))
            })
            .when(self.calibration_dialog.is_some(), |body| {
                body.child(self.render_calibration_error(window, cx))
            })
            .when(self.calibration_popup.is_some(), |body| {
                body.child(self.render_calibration_popup(window, cx))
            })
            .when(self.trigger_calibration_popup.is_some(), |body| {
                body.child(self.render_trigger_calibration_popup(window, cx))
            })
    }
}

fn range_tip_label(value: i64) -> Div {
    div()
        .flex_shrink_0()
        .whitespace_nowrap()
        .rounded(surface::css(3.))
        .bg(rgb(0x44d62c))
        .px(surface::css(9.))
        .py(surface::css(4.))
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .text_color(rgb(0x000000))
        .child(value.to_string())
}

/// Restore fields with an audited schema. Hardware action logs never become
/// controller acknowledgements merely because a local profile contains them.
fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if matches!(
                    key.as_str(),
                    "actionsFromUI" | "actionsFromLocalStorage" | "errorActions"
                ) {
                    continue;
                }
                if let Some(saved) = saved.get(key) {
                    merge_known(value, saved);
                }
            }
        }
        (target @ Value::Array(_), Value::Array(_))
        | (target @ Value::Bool(_), Value::Bool(_))
        | (target @ Value::Number(_), Value::Number(_))
        | (target @ Value::String(_), Value::String(_)) => *target = saved.clone(),
        _ => {}
    }
}

// Generated catalog reader: razer-pages/gamepad_products_data.
fn read_gamepad_products_catalog() -> Vec<(u32, fn() -> GamepadProductSpec)> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<GamepadProductSpec> {
        let value = match r.tag()? {
            0 => GamepadProductSpec {
                product_id: r.child(decode_1)?,
                profile: r.child(decode_2)?,
                controller: r.child(decode_2)?,
                pages: r.child(decode_3)?,
                button_controls: r.child(decode_5)?,
                assignments: r.child(decode_3)?,
                arcade: r.child(decode_6)?,
                socd_modes: r.child(decode_7)?,
                trigger_reset: r.child(decode_2)?,
                analog_mode: r.child(decode_8)?,
                digital_mode: r.child(decode_8)?,
                standard_mode: r.child(decode_8)?,
                circular_mode: r.child(decode_8)?,
                deadzone_steps: r.child(decode_9)?,
                sensitivity_steps: r.child(decode_9)?,
                power_minutes: r.child(decode_9)?,
                brightness_step: None,
                controller_lighting: r.child(decode_6)?,
                effects: r.child(decode_5)?,
                polling_rates: r.child(decode_9)?,
                info: r.child(decode_2)?,
            },
            1 => GamepadProductSpec {
                product_id: r.child(decode_1)?,
                profile: r.child(decode_2)?,
                controller: r.child(decode_2)?,
                pages: r.child(decode_3)?,
                button_controls: r.child(decode_5)?,
                assignments: r.child(decode_3)?,
                arcade: r.child(decode_6)?,
                socd_modes: r.child(decode_7)?,
                trigger_reset: r.child(decode_2)?,
                analog_mode: r.child(decode_8)?,
                digital_mode: r.child(decode_8)?,
                standard_mode: r.child(decode_8)?,
                circular_mode: r.child(decode_8)?,
                deadzone_steps: r.child(decode_9)?,
                sensitivity_steps: r.child(decode_9)?,
                power_minutes: r.child(decode_9)?,
                brightness_step: r.child(decode_10)?,
                controller_lighting: r.child(decode_6)?,
                effects: r.child(decode_5)?,
                polling_rates: r.child(decode_9)?,
                info: r.child(decode_2)?,
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
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
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
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<i64> {
        let value = match r.tag()? {
            0 => r.i64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<i64>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<f32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_11)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn product_2629() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2629u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2636() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2636u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2647() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2647u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2650() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2650u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2676() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2676u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2684() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            2684u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4115() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            4115u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4133() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            4133u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4144() -> GamepadProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_GAMEPAD_PRODUCTS_DATA,
            4144u32,
            "0a9b90529f1665b2c3a965faf03068bd0995ccfd8748c8bcb654aa882c2c1ad0",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    vec![
        (2629u32, product_2629 as fn() -> GamepadProductSpec),
        (2636u32, product_2636 as fn() -> GamepadProductSpec),
        (2647u32, product_2647 as fn() -> GamepadProductSpec),
        (2650u32, product_2650 as fn() -> GamepadProductSpec),
        (2676u32, product_2676 as fn() -> GamepadProductSpec),
        (2684u32, product_2684 as fn() -> GamepadProductSpec),
        (4115u32, product_4115 as fn() -> GamepadProductSpec),
        (4133u32, product_4133 as fn() -> GamepadProductSpec),
        (4144u32, product_4144 as fn() -> GamepadProductSpec),
    ]
}

#[cfg(test)]
mod gamepad_products_data_catalog_tests {
    #[test]
    fn packaged_catalog_gamepad_products_data_decodes() {
        for (_, factory) in super::read_gamepad_products_catalog() {
            let _ = factory();
        }
    }
}
// End generated catalog reader: razer-pages/gamepad_products_data.
