//! Native controller pages from each current product's mounted components.
//! `profile` and the independent Redux controller states remain separate. These
//! are local drafts; service acknowledgements and live tester data are not faked.
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    Disableable, Selectable, StyledExt,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
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
#[path = "gamepad_deadzone_dialog.rs"]
mod deadzone_dialog;
#[path = "kitsune.rs"]
mod kitsune;

#[derive(Deserialize)]
pub struct GamepadProductSpec {
    product_id: u32,
    name: String,
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
    static PRODUCTS: OnceLock<Vec<GamepadProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("gamepad_products_data.json"))
                .expect("validated current gamepad specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}

pub struct GamepadProductChanged;
pub struct GamepadCalibrationRequested;

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
    sensitivity: bool,
    low_deadzone: Option<(String, Value)>,
    previous_deadzones: BTreeMap<String, Value>,
    deadzone_dialog: Option<deadzone_dialog::DialogState>,
    calibration_state: calibration::state::CalibrationState,
    calibration_dialog: Option<calibration::CalibrationDialog>,
    calibration_popup: Option<calibration::CalibrationDialog>,
    calibration_bounds: Rc<Cell<Bounds<Pixels>>>,
    thumbstick_bounds: Rc<Cell<Bounds<Pixels>>>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<GamepadProductChanged> for GamepadProductWorkspace {}
impl EventEmitter<GamepadCalibrationRequested> for GamepadProductWorkspace {}
impl EventEmitter<CalibrationIntent> for GamepadProductWorkspace {}

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
            sensitivity: false,
            low_deadzone: None,
            previous_deadzones: BTreeMap::new(),
            deadzone_dialog: None,
            calibration_state: Default::default(),
            calibration_dialog: None,
            calibration_popup: None,
            calibration_bounds: Rc::new(Cell::new(Bounds::default())),
            thumbstick_bounds: Rc::new(Cell::new(Bounds::default())),
            sliders: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
        };
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
        this.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_range_edits(cx);
                }
            }));
        this
    }

    pub fn set_page(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.cancel_range_edits(cx);
            if self.page == "TAB_CALIBRATION" || self.calibration_popup.is_some() {
                self.leave_calibration(window, cx);
            }
            // 2636's mounted thumbstick component initializes prevLeft/Right
            // once on entry. Continue does not replace these rollback values.
            if self.spec.product_id == 2636 && key == "THUMBSTICKS" {
                self.previous_deadzones.clear();
                for side in ["leftStick", "rightStick"] {
                    let path = format!("/controller/{side}/deadzoneValue");
                    if let Some(value) = self.draft.pointer(&path) {
                        self.previous_deadzones.insert(path, value.clone());
                    }
                }
            }
            self.page = key.into();
            self.dismiss_deadzone(false, window, cx);
            self.low_deadzone = None;
            cx.notify();
        }
    }

    pub fn snapshot(&self) -> Value {
        self.draft.clone()
    }

    pub fn restore(&mut self, saved: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_range_edits(cx);
        self.leave_calibration(window, cx);
        self.draft = json!({"profile": self.spec.profile, "controller": self.spec.controller});
        if let Some(saved) =
            saved.filter(|v| v["profile"].is_object() && v["controller"].is_object())
        {
            merge_known(&mut self.draft, saved);
        }
        self.selected_button = None;
        self.dismiss_deadzone(false, window, cx);
        self.low_deadzone = None;
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

    fn toggle(&self, path: &str, label: String, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let path = path.to_owned();
        Checkbox::new(SharedString::from(format!("gamepad-{path}")))
            .label(label)
            .checked(self.checked(&path))
            .disabled(!enabled)
            .on_click(cx.listener(move |this, value, _, cx| {
                if enabled {
                    this.write(&path, json!(value), cx);
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

    fn range(&self, path: &str, label: String, enabled: bool) -> AnyElement {
        let Some(slider) = self.sliders.get(path) else {
            return div().into_any_element();
        };
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .child(label)
                    .child(format!("{}%", self.number(path))),
            )
            .child(Slider::new(slider).disabled(!enabled))
            .into_any_element()
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
                    .bg(if pressed {
                        rgb(0x383838)
                    } else {
                        rgb(0x44d62c)
                    })
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
                .child(
                    div()
                        .rounded(surface::css(3.))
                        .bg(rgb(0x44d62c))
                        .px(surface::css(9.))
                        .py(surface::css(4.))
                        .font_family("Roboto")
                        .text_size(surface::css(12.))
                        .text_color(rgb(0x000000))
                        .child(value.to_string()),
                )
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
            value: if handle == RangeHandle::Start {
                start
            } else {
                end
            },
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
        let current = if handle == RangeHandle::Start {
            start
        } else {
            end
        };
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
            cx.emit(GamepadProductChanged);
            cx.notify();
        }
    }

    /// `JP`：每个扳机的组件。标题是 `LEFT_TRIGGER_MODE`/`RIGHT_TRIGGER_MODE`
    /// （`we.LST`/`we.gzX`），`tips` 是同一个提示键；组件体是 `.radioList`
    /// （`display:grid`）里的 Analog/Digital 两个 `radioItem` 勾选项，然后按模式分支：
    /// 模拟模式显示 `.h1-body` = `<SIDE>_TRIGGER_RANGE`（`we.Vwf`/`we.ukk`）与
    /// `QP` 双柄滑条（外层 `margin-top:35px`），数字模式显示
    /// `<SIDE>_ACTUATION_POINT`（`we.C7E`/`we.Fh7`）与 1–100 的滑条
    /// （`minTag:"1%"`、`maxTag:"100%"`、`tipFormat` 加 `%`）。两者都带
    /// `.reset-actuation` 重置链接，仅在取值偏离默认时可用。
    fn triggers(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .gap_5()
            .child(surface::note(t("ACTUATION_DESC"), cx))
            .child(
                surface::page_columns()
                    .child(surface::page_column(self.trigger_panel(
                        "leftTrigger",
                        "LEFT",
                        cx,
                    )))
                    .child(surface::page_column(self.trigger_panel(
                        "rightTrigger",
                        "RIGHT",
                        cx,
                    ))),
            )
            // 原版按设备信息里的 `minFWSupportTriggers` 显示固件要求提示。
            .when_some(
                self.spec.info["minFWSupportTriggers"].as_str(),
                |view, version| {
                    view.child(surface::note(
                        format!("此产品扳机功能要求固件 {version}；当前未读取设备固件。"),
                        cx,
                    ))
                },
            )
            .into_any_element()
    }

    fn trigger_panel(&self, side: &str, prefix: &str, cx: &Context<Self>) -> AnyElement {
        let panel = {
            let (mode_key, range_key, point_key) = trigger_keys(prefix);
            let root = format!("/controller/{side}");
            let analog = self.number(&format!("{root}/mode")) == self.spec.analog_mode;
            let reset_fields: &[&str] = if analog {
                &["startRange", "endRange"]
            } else {
                &["actuationPoint", "isRapidTrigger"]
            };
            let changed = reset_fields.iter().any(|field| {
                self.draft.pointer(&format!("{root}/{field}")) != self.spec.trigger_reset.get(field)
            });
            let mut panel = surface::panel(t(mode_key), cx).child(
                v_flex()
                    .child(self.choice(
                        &format!("{root}/mode"),
                        json!(self.spec.analog_mode),
                        t("ANALOG"),
                        true,
                        cx,
                    ))
                    .child(self.choice(
                        &format!("{root}/mode"),
                        json!(self.spec.digital_mode),
                        t("DIGITAL"),
                        true,
                        cx,
                    )),
            );
            let label_key = if analog { range_key } else { point_key };
            let mut branch = h_flex().justify_between().items_center().child(
                // `.h1-body{color:#ccc;margin-bottom:10px}`，原版这里把
                // `margin-bottom` 覆盖成 0、并让元素 `display:inline-block`。
                div().text_color(rgb(0xcccccc)).child(t(label_key)),
            );
            branch = branch.child(
                // `.reset-actuation{font-size:14px;position:absolute;right:35px;
                //  text-decoration:underline;text-transform:capitalize}` 与
                // `:hover{color:#44d62c}`、`.disabled{opacity 由调用方控制}`。
                // 图标 `icon_reset.f416d0b7.svg` 不在已抓取的设备包里，因此只渲染文字。
                BaseButton::new(SharedString::from(format!("gamepad-reset-{side}")))
                    .accessibility_label(t("RESET"))
                    .disabled(!changed)
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .underline()
                    .when(!changed, |link| link.opacity(0.3))
                    .hover(|style| style.text_color(rgb(0x44d62c)))
                    .on_click({
                        let reset_root = root.clone();
                        cx.listener(move |this, _, window, cx| {
                            if !changed {
                                return;
                            }
                            this.reset_trigger(&reset_root, window, cx);
                        })
                    })
                    .child(t("RESET")),
            );
            if analog {
                let start = self.number(&format!("{root}/startRange"));
                let end = self.number(&format!("{root}/endRange"));
                panel = panel.child(branch).child(
                    div()
                        .mt(surface::css(35.))
                        .child(self.range_control(&root, start, end, cx)),
                );
            } else {
                panel = panel
                    .child(branch)
                    .child(
                        div().mt(surface::css(20.)).child(
                            // `<slider min={1} max={100} step={1} minTag="1%"
                            //  maxTag="100%" tipFormat={v => v + "%"}>`
                            v_flex()
                                .child(self.range(
                                    &format!("{root}/actuationPoint"),
                                    t(point_key),
                                    true,
                                ))
                                .child(surface::slider_tags("1%", None, "100%", None)),
                        ),
                    )
                    .child(self.toggle(
                        &format!("{root}/isRapidTrigger"),
                        t("RAPID_TRIGGER"),
                        true,
                        cx,
                    ));
            }
            panel
        };
        panel.into_any_element()
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
        let mut left = surface::panel(t("TAB_CUSTOMIZE"), cx).child(self.spec.name.clone());
        if !self.spec.arcade {
            left = left.child(h_flex().gap_2().flex_wrap().children(
                self.spec.button_controls.iter().filter_map(|button| {
                    let id = button["inputID"].as_str()?.to_owned();
                    let label = button["counter"].as_str().unwrap_or(&id).to_owned();
                    Some(
                        Button::new(SharedString::from(format!("gamepad-input-{id}")))
                            .label(label)
                            .outline()
                            .selected(self.selected_button.as_ref() == Some(&id))
                            .disabled(button["isEnabled"].as_bool() == Some(false))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_button = Some(id.clone());
                                cx.notify();
                            })),
                    )
                }),
            ));
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
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
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

    fn thumbsticks(&self, cx: &Context<Self>) -> AnyElement {
        let bounds_cell = self.thumbstick_bounds.clone();
        let mut page = v_flex().gap_5().child(h_flex().gap_2().children(
            [(false, "DEADZONE"), (true, "SENSITIVITY_CLUTCH")].map(|(sensitivity, label)| {
                Button::new(SharedString::from(format!("gamepad-stick-tab-{label}")))
                    .label(t(label))
                    .outline()
                    .selected(self.sensitivity == sensitivity)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sensitivity = sensitivity;
                        cx.notify();
                    }))
            }),
        ));
        let field = if self.sensitivity {
            "sensitivityValue"
        } else {
            "deadzoneValue"
        };
        let steps = if self.sensitivity {
            &self.spec.sensitivity_steps
        } else {
            &self.spec.deadzone_steps
        };
        let panels = [
            ("leftStick", "LEFT_THUMBSTICK"),
            ("rightStick", "RIGHT_THUMBSTICK"),
        ]
        .map(|(side, label)| {
            let path = format!("/controller/{side}/{field}");
            let enabled = !self.sensitivity || self.clutch_assigned(side);
            surface::panel(t(label), cx)
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .children(steps.iter().map(|value| {
                            let path = path.clone();
                            let value = *value;
                            Button::new(SharedString::from(format!("gamepad-{path}-{value}")))
                                .label(format!("{value}%"))
                                .outline()
                                .disabled(!enabled)
                                .selected(self.number(&path) == value)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if !enabled {
                                        return;
                                    }
                                    if !this.sensitivity && value < 7 && this.number(&path) != value
                                    {
                                        let previous = if this.spec.product_id == 2636 {
                                            this.previous_deadzones.get(&path)
                                        } else {
                                            None
                                        }
                                        .or_else(|| this.draft.pointer(&path))
                                        .cloned()
                                        .expect("source deadzone");
                                        this.low_deadzone = Some((path.clone(), previous));
                                        if this.spec.product_id == 2636 {
                                            this.deadzone_dialog =
                                                Some(deadzone_dialog::DialogState::new(window, cx));
                                        }
                                    }
                                    if this.spec.product_id == 2636
                                        && !this.sensitivity
                                        && value >= 7
                                    {
                                        this.previous_deadzones.insert(path.clone(), json!(value));
                                    }
                                    this.write(&path, json!(value), cx);
                                }))
                        })),
                )
                .when(!enabled, |p| {
                    p.child(surface::note(t("SENSITIVITY_ASSIGN_INFO"), cx))
                })
                .when(
                    !self.sensitivity && matches!(self.spec.product_id, 2676 | 2684),
                    |p| {
                        let part = if side == "leftStick" { 1 } else { 2 };
                        p.child(
                            Button::new(SharedString::from(format!(
                                "gamepad-stick-calibrate-{part}"
                            )))
                            .label(t("CALIBRATE"))
                            .outline()
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.open_calibration_popup(part, window, cx)
                                },
                            )),
                        )
                    },
                )
        });
        page = page
            .child(surface::page_columns().children(panels.into_iter().map(surface::page_column)));
        if !self.sensitivity {
            page = page.child(
                surface::page_columns()
                    .child(surface::page_column(
                        surface::panel(t("CIRCULARITY_MODE_HEADER"), cx)
                            .child(self.choice(
                                "/controller/circularity/mode",
                                json!(self.spec.standard_mode),
                                t("STANDARD"),
                                true,
                                cx,
                            ))
                            .child(self.choice(
                                "/controller/circularity/mode",
                                json!(self.spec.circular_mode),
                                t("CIRCULAR"),
                                true,
                                cx,
                            )),
                    ))
                    .child(surface::page_column(
                        surface::panel(t("PREVENT_DOUBLE_DEADZONES"), cx)
                            .child(
                                Checkbox::new("gamepad-prevent-double-deadzone")
                                    .label(t("PREVENT_DOUBLE_DEADZONES"))
                                    .checked(
                                        self.checked("/controller/leftStick/preventDoubleDeadzone")
                                            && self.checked(
                                                "/controller/rightStick/preventDoubleDeadzone",
                                            ),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        for side in ["leftStick", "rightStick"] {
                                            let path =
                                                format!("/controller/{side}/preventDoubleDeadzone");
                                            this.write(&path, json!(!this.checked(&path)), cx);
                                        }
                                    })),
                            )
                            .child(surface::note(t("PREVENT_DOUBLE_DEADZONES_DESCRIPTION"), cx)),
                    )),
            );
        }
        if let Some((path, previous)) = self
            .low_deadzone
            .as_ref()
            .filter(|_| self.spec.product_id != 2636)
        {
            let path = path.clone();
            let previous = previous.clone();
            page = page.child(
                surface::panel(t("LOW_DEADZONE_INFO_TITLE"), cx)
                    .child(surface::note(t("LOW_DEADZONE_INFO_DETAIL"), cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("gamepad-low-deadzone-continue")
                                    .label(t("CONTINUE"))
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.low_deadzone = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("gamepad-low-deadzone-revert")
                                    .label(t("CANCEL"))
                                    .outline()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.write(&path, previous.clone(), cx);
                                        this.low_deadzone = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        page.relative()
            .child(
                canvas(
                    move |bounds, window, _| {
                        if bounds_cell.replace(bounds) != bounds {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
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
        let enabled = self.checked("/profile/brightness/isEnabled");
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .child(
                        surface::panel_with_title_switch(
                            t("BRIGHTNESS_HEADER"),
                            surface::SynapseSwitch::new("gamepad-brightness")
                                .accessibility_label(t("BRIGHTNESS_HEADER"))
                                .checked(enabled)
                                .on_change(cx.listener(|this, next: &bool, _, cx| {
                                    this.write("/profile/brightness/isEnabled", json!(*next), cx);
                                })),
                            surface::help_control(
                                "gamepad-brightness-help",
                                t("BRIGHTNESS_TOOLTIP"),
                            ),
                            cx,
                        )
                        .child(surface::slider_tags("0", None, "100", None))
                        .child(self.range(
                            "/profile/brightness/value",
                            razer_i18n::t("BRIGHTNESS_HEADER"),
                            enabled,
                        )),
                    )
                    .when(!self.spec.controller_lighting, |column| {
                        column.child(self.switch_off_lighting(window, cx))
                    }),
            ))
            .child(surface::page_column({
                let path = if self.spec.controller_lighting {
                    "/controller/lighting/effectId"
                } else {
                    "/profile/quickEffects/selectedEffectId"
                };
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
        let widget = surface::panel_with_control(
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
                column = column.child(Slider::new(slider).disabled(!(brightness_on && idle_on)));
            }
            column.child(surface::slider_tags("1", None, "15", None))
        });
        widget.into_any_element()
    }

    fn power(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_control(
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
        if self.has_page_calibration() {
            return self.render_calibration(window, cx);
        }
        surface::panel(t("TAB_CALIBRATION"), cx)
            .child(surface::note(t("CALIBRATION_STEP0"), cx))
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("gamepad-calibrate-left")
                            .label(t("CALIBBRTION_LEFT_THUMBSITCK"))
                            .outline()
                            .disabled(true),
                    )
                    .child(
                        Button::new("gamepad-calibrate-right")
                            .label(t("CALIBBRTION_RIGHT_THUMBSITCK"))
                            .outline()
                            .disabled(true),
                    ),
            )
            .child(surface::note(
                "未连接手柄校准服务，无法开始校准或读取摇杆位置。",
                cx,
            ))
            .into_any_element()
    }
}

impl Render for GamepadProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page.as_str() {
            "TAB_CUSTOMIZE" => self.customize(cx),
            "TRIGGERS" => self.triggers(cx),
            "THUMBSTICKS" => self.thumbsticks(cx),
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
    }
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
