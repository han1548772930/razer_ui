//! Native controller pages from each current product's mounted components.
//! `profile` and the independent Redux controller states remain separate. These
//! are local drafts; service acknowledgements and live tester data are not faked.
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

use crate::{i18n::t, ui::surface};

#[derive(Deserialize)]
pub(crate) struct GamepadProductSpec {
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

pub(crate) fn source_product(pid: u32) -> Option<&'static GamepadProductSpec> {
    static PRODUCTS: OnceLock<Vec<GamepadProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("gamepad_products_data.json"))
                .expect("validated current gamepad specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}

pub(crate) struct GamepadProductChanged;

/// 源码 `GR` 的功耗取值标签：`item.value >= 60 ? ra.pHP : ra.yvH`，同时
/// `data.value` 在 `>= 60` 时除以 60。两个别名经导出表解析为 `MIN`
/// （"{{value}} min."）与 `SEC`（"{{value}} sec."），说明该组件按**秒**判断：
/// 不足 60 显示秒、满 60 起按分钟显示。
pub(crate) fn power_saving_label(value: i64) -> String {
    if value >= 60 {
        crate::i18n::t_value("MIN", value / 60)
    } else {
        crate::i18n::t_value("SEC", value)
    }
}

/// `JP` 按扳机侧选的三对键：`we.LST`/`we.gzX`（标题）、`we.Vwf`/`we.ukk`
/// （模拟分支的 `.h1-body`）、`we.C7E`/`we.Fh7`（数字分支的 `.h1-body`）。
pub(crate) fn trigger_keys(prefix: &str) -> (&'static str, &'static str, &'static str) {
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
pub(crate) fn range_handle_value(handle: RangeHandle, value: i64, start: i64, end: i64) -> i64 {
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
pub(crate) enum RangeHandle {
    Start,
    End,
}

/// `QP` 的几何与拖拽状态：`.rangeSlider` 的容器 bounds 由 canvas 记录，指针位置
/// 按容器宽度换算成 0–100 的整数。
#[derive(Clone, Default)]
struct RangeDrag {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    handle: Option<RangeHandle>,
}

pub(crate) struct GamepadProductWorkspace {
    spec: &'static GamepadProductSpec,
    range_drag: RangeDrag,
    page: String,
    draft: Value,
    selected_button: Option<String>,
    sensitivity: bool,
    low_deadzone: Option<(String, Value)>,
    sliders: BTreeMap<String, Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}

impl EventEmitter<GamepadProductChanged> for GamepadProductWorkspace {}

impl GamepadProductWorkspace {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited gamepad product");
        let mut this = Self {
            spec,
            range_drag: RangeDrag::default(),
            page: "TAB_CUSTOMIZE".into(),
            draft: json!({"profile": spec.profile, "controller": spec.controller}),
            selected_button: None,
            sensitivity: false,
            low_deadzone: None,
            sliders: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
        };
        if spec.pages.iter().any(|p| p == "TRIGGERS") {
            for side in ["leftTrigger", "rightTrigger"] {
                for (field, min) in [("startRange", 0.), ("endRange", 0.), ("actuationPoint", 1.)] {
                    this.add_slider(
                        &format!("/controller/{side}/{field}"),
                        min,
                        100.,
                        1.,
                        window,
                        cx,
                    );
                }
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
        this
    }

    pub(crate) fn set_page(&mut self, key: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.page != key {
            self.page = key.into();
            self.low_deadzone = None;
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
        self.draft = json!({"profile": self.spec.profile, "controller": self.spec.controller});
        if let Some(saved) =
            saved.filter(|v| v["profile"].is_object() && v["controller"].is_object())
        {
            merge_known(&mut self.draft, saved);
        }
        self.selected_button = None;
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
                    let mut value = value.start().clamp(min, max).round() as i64;
                    // The source's dual range control cannot cross its other handle.
                    if key.ends_with("/startRange") {
                        let other = key.replace("/startRange", "/endRange");
                        value = value.min(this.number(&other));
                    } else if key.ends_with("/endRange") {
                        let other = key.replace("/endRange", "/startRange");
                        value = value.max(this.number(&other));
                    }
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
        let bounds_cell = self.range_drag.bounds.clone();
        // 原版用 `:active` 表示手柄被按住；本地按拖拽状态给同样的
        // `background:#383838;border:2px solid #44d62c`。
        let thumb = |value: i64, handle: RangeHandle| {
            let pressed = self.range_drag.handle == Some(handle);
            div()
                .absolute()
                .left(relative(value as f32 / 100.))
                .top(surface::css(11.))
                .w_0()
                .flex()
                .justify_center()
                .child(
                    div()
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
                        .hover(|style| {
                            style
                                .bg(rgb(0x5d5d5d))
                                .border_2()
                                .border_color(rgb(0x44d62c))
                        }),
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
        let root_down = root.to_owned();
        let root_move = root.to_owned();
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
                        canvas(move |bounds, _, _| bounds_cell.set(bounds), |_, _, _, _| ())
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
                    .child(thumb(end, RangeHandle::End))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.range_drag_start(event, &root_down, cx)
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        this.range_drag_move(event, &root_move, cx)
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.range_drag.handle = None),
                    ),
            )
            .child(h_flex().justify_between().child("0").child("100"))
            .into_any_element()
    }

    fn range_pointer_value(&self, x: Pixels) -> i64 {
        let bounds = self.range_drag.bounds.get();
        let width = f32::from(bounds.size.width).max(1.);
        let local = f32::from(x - bounds.left()) / width;
        (local * 100.).round().clamp(0., 100.) as i64
    }

    /// 原版是两个重叠的 `<input type=range>`，由浏览器命中决定拖哪一个；本地按
    /// 「离哪个手柄更近」选择。
    fn range_drag_start(&mut self, event: &MouseDownEvent, root: &str, cx: &mut Context<Self>) {
        let start = self.number(&format!("{root}/startRange"));
        let end = self.number(&format!("{root}/endRange"));
        let value = self.range_pointer_value(event.position.x);
        self.range_drag.handle = Some(if (value - start).abs() <= (value - end).abs() {
            RangeHandle::Start
        } else {
            RangeHandle::End
        });
        self.range_drag_apply(root, value, cx);
    }

    fn range_drag_move(&mut self, event: &MouseMoveEvent, root: &str, cx: &mut Context<Self>) {
        if self.range_drag.handle.is_none() {
            return;
        }
        let value = self.range_pointer_value(event.position.x);
        self.range_drag_apply(root, value, cx);
    }

    fn range_drag_apply(&mut self, root: &str, value: i64, cx: &mut Context<Self>) {
        let Some(handle) = self.range_drag.handle else {
            return;
        };
        let start_path = format!("{root}/startRange");
        let end_path = format!("{root}/endRange");
        let (start, end) = (self.number(&start_path), self.number(&end_path));
        let next = range_handle_value(handle, value, start, end);
        match handle {
            RangeHandle::Start if next != start => self.write(&start_path, json!(next), cx),
            RangeHandle::End if next != end => self.write(&end_path, json!(next), cx),
            _ => {}
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
                div()
                    .id(SharedString::from(format!("gamepad-reset-{side}")))
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
                            let fields: &[&str] = if analog {
                                &["startRange", "endRange"]
                            } else {
                                &["actuationPoint", "isRapidTrigger"]
                            };
                            for field in fields {
                                if let Some(value) = this.spec.trigger_reset.get(*field) {
                                    this.write(&format!("{reset_root}/{field}"), value.clone(), cx);
                                }
                            }
                            this.sync_sliders(window, cx);
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
    pub(crate) fn customize_element(&self, cx: &mut Context<Self>) -> AnyElement {
        self.customize(cx)
    }

    fn customize(&self, cx: &Context<Self>) -> AnyElement {
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
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !enabled {
                                        return;
                                    }
                                    if !this.sensitivity && value < 7 && this.number(&path) != value
                                    {
                                        this.low_deadzone = Some((
                                            path.clone(),
                                            this.draft
                                                .pointer(&path)
                                                .cloned()
                                                .expect("source deadzone"),
                                        ));
                                    }
                                    this.write(&path, json!(value), cx);
                                }))
                        })),
                )
                .when(!enabled, |p| {
                    p.child(surface::note(t("SENSITIVITY_ASSIGN_INFO"), cx))
                })
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
        if let Some((path, previous)) = &self.low_deadzone {
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
        page.into_any_element()
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
                            crate::i18n::t("BRIGHTNESS_HEADER"),
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

    fn calibration(&self, cx: &Context<Self>) -> AnyElement {
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
            "TAB_CALIBRATION" => self.calibration(cx),
            _ => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        };
        super::product_surface::body().child(content)
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
