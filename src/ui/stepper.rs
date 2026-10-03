//! 共享数字步进器（`.stepper`）。
//!
//! 当前源码里这个组件被相机页与 ARGB 页共用，行为与像素都取自产品包 CSS 与 JS
//! （3594／3595／3871 是同一份实现）：
//!
//! - 盒模型：`60x27` 外框、`1px solid #5d5d5d`；输入区 `58x25`（`#111` 背景、
//!   `#ccc` 文字、`14px/17px`、`padding:5px 18px 5px 5px`）；
//! - 上下箭头：`14x12`、`background-position:50%`、`background-size:8px`，上箭头
//!   贴顶（`background-position-y:5px`）、下箭头贴底（`3px`）；默认不可见，在
//!   `.stepper:hover`／`:focus-within` 时 `0.1s linear` 淡入；箭头 hover 背景
//!   `#ffffff1a`、按下 `#0000001a`；
//! - 交互：按下立即走一步，随后每 `300ms` 重复一次（`setInterval(()=>{e()},300)`），
//!   松开或移出停止；禁用时整块 `opacity:.3;pointer-events:none`。
//!
//! 箭头图标是产品包里同名同哈希的两份 SVG（`stepper_up.dcb04520.svg`、
//! `stepper_down.349f755c.svg`），已按原样打包为
//! `synapse/wired-argb-3871-stepper_up.svg` / `_down.svg`。
use crate::ui::{surface, theme::CameraProductColors as Colors};
use gpui_kit::*;
use std::{cell::Cell, rc::Rc, time::Duration};

/// 原版 `setInterval(()=>{e()},300)` 的重复间隔。
const REPEAT: Duration = Duration::from_millis(300);

/// 步进器每次改值都会发出这个事件；父视图在自己的 `Context` 里处理。
#[derive(Clone, Copy, Debug)]
pub(crate) struct StepperEvent {
    pub value: f64,
}
impl EventEmitter<StepperEvent> for Stepper {}

pub(crate) struct Stepper {
    id: SharedString,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    /// `allowDecimal` + `roundUpDecimals` 决定显示几位小数。
    decimals: usize,
    disabled: bool,
    /// 按住箭头期间为真；松开即置假，重复任务据此退出。
    holding: Rc<Cell<bool>>,
    task: Option<Task<()>>,
}
impl Stepper {
    pub(crate) fn new(id: impl Into<SharedString>, value: f64) -> Self {
        Self {
            id: id.into(),
            value,
            min: 0.,
            max: 100.,
            step: 1.,
            decimals: 0,
            disabled: false,
            holding: Rc::new(Cell::new(false)),
            task: None,
        }
    }
    pub(crate) fn range(mut self, min: f64, max: f64, step: f64) -> Self {
        self.min = min;
        self.max = max;
        self.step = step;
        self
    }
    pub(crate) fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }
    fn clamp(&self, value: f64) -> f64 {
        let stepped = (value / self.step).round() * self.step;
        ((stepped / self.step).round() * self.step).clamp(self.min, self.max)
    }
    fn formatted(&self) -> String {
        if self.decimals == 0 {
            format!("{}", self.value.round() as i64)
        } else {
            format!("{:.*}", self.decimals, self.value)
        }
    }
    /// 原版 `onMouseDown`：立即走一步并开始每 300ms 重复；`onMouseUp` 停止。
    fn press(&mut self, delta: f64, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.clamp(self.value + delta * self.step);
        self.set_value(value, cx);
        self.holding.set(true);
        let holding = self.holding.clone();
        let (min, max, step) = (self.min, self.max, self.step);
        let current = Rc::new(Cell::new(value));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor().timer(REPEAT).await;
                if !holding.get() {
                    break;
                }
                let next = (current.get() + delta * step).clamp(min, max);
                if (next - current.get()).abs() < f64::EPSILON {
                    // 已经到边界：原版仍会重复触发，但值不再变化。
                    continue;
                }
                current.set(next);
                if view
                    .update(cx, |stepper, cx| stepper.set_value(next, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        self.value = value;
        cx.emit(StepperEvent { value });
        cx.notify();
    }
    /// 父视图同步草稿值：步进器本身不改状态，只显示当前值与禁用态。
    pub(crate) fn sync_value(&mut self, value: f64, disabled: bool, cx: &mut Context<Self>) {
        let changed = (self.value - value).abs() > f64::EPSILON || self.disabled != disabled;
        self.value = value;
        self.disabled = disabled;
        if changed {
            cx.notify();
        }
    }
    fn release(&mut self, cx: &mut Context<Self>) {
        self.holding.set(false);
        self.task = None;
        cx.notify();
    }
    fn spinner(
        &self,
        which: &'static str,
        icon: &'static str,
        delta: f64,
        top: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let group = SharedString::from(format!("{}-spinner", which));
        let id = SharedString::from(format!("{}:{}", self.id, which));
        let mut spinner = div()
            .id(id)
            .absolute()
            .right_0()
            .top(surface::css(top))
            .w(surface::css(14.))
            .h(surface::css(12.))
            .group(group.clone())
            // `.icon.spinner{opacity:0;visibility:hidden}` → `.stepper:hover` 时显示。
            .opacity(0.)
            .group_hover(self.id.clone(), |spinner| spinner.opacity(1.))
            .hover(|spinner| spinner.bg(gpui_kit::rgba(0xffffff1a)))
            .child(
                img(icon)
                    .absolute()
                    .left(surface::css(3.))
                    .top(surface::css(2.))
                    .w(surface::css(8.))
                    .h(surface::css(8.)),
            );
        if !self.disabled {
            spinner = spinner
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| this.press(delta, window, cx)),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.release(cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.release(cx)),
                );
        }
        spinner.into_any_element()
    }
}
impl Render for Stepper {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.formatted();
        let mut root = div()
            .relative()
            .w(surface::css(60.))
            .h(surface::css(27.))
            .border_1()
            .border_color(Colors::border());
        if self.disabled {
            // `.stepper.disabled{opacity:.3;pointer-events:none}`
            root = root.opacity(0.3);
        }
        root.id(self.id.clone())
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(surface::css(58.))
                    .h(surface::css(25.))
                    .bg(Colors::background())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::text())
                    .px(surface::css(5.))
                    .py(surface::css(5.))
                    .child(text),
            )
            .child(self.spinner("up", "synapse/wired-argb-3871-stepper_up.svg", 1., 0., cx))
            .child(self.spinner(
                "down",
                "synapse/wired-argb-3871-stepper_down.svg",
                -1.,
                15.,
                cx,
            ))
    }
}
