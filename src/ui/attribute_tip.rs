//! 全局 `[tooltip]` 属性徽标：源里用 `tooltip={…}` / `tooltip="文本"` 的属性提示，
//! 由伪元素 `[tooltip]:before` 呈现（不含任何展示延迟，只做 300ms 线性淡入）。
//!
//! 当前源（691 `static/css/5171.1330bdc6.chunk.css` 的全局规则）：
//!
//! ```css
//! [tooltip]:before{background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;
//!   color:#ccc;content:attr(tooltip);display:block;font-size:14px;height:auto;line-height:16px;
//!   opacity:0;padding:8px 10px;pointer-events:none;position:absolute;right:0;text-align:left;
//!   top:calc(100% + 5px);transition:visibility 0s,opacity .3s linear;visibility:hidden;
//!   white-space:nowrap;width:auto;z-index:100}
//! [tooltip]:hover:before{opacity:1;visibility:visible}
//! ```
//!
//! 注意有更具体的覆盖规则（例如 `.indicator--item[tooltip]:before{right:auto;
//! top:calc(100% + 10px)}`、`.nav-tabs .batt[tooltip]:before{…margin-right:16px…}`、
//! `.device--badge[tooltip]:before{display:none}`），本模块只实现**没有任何覆盖**时的全局形态；
//! 有覆盖的页面应使用各自的 kind。调用方负责在带 `tooltip` 属性的元素上维护 hover 状态，
//! 悬停时把本元素挂进该元素（源里伪元素正是挂在同一个元素上，位置相对该元素计算）。
use crate::ui::{surface, theme::TooltipColors};
use gpui_kit::*;
use std::time::Duration;

/// 悬停时立即挂载的 `[tooltip]:before` 徽标：贴目标下沿 5px、右缘对齐、不换行，300ms 线性淡入。
///
/// 调用方持有 hover 状态时用它（对应源伪元素靠 `:hover` 出现 + `transition:opacity .3s linear`）。
pub(crate) fn attribute_tip(
    id: impl Into<ElementId> + Clone,
    text: impl Into<SharedString>,
) -> AnyElement {
    let text: SharedString = text.into();
    div()
        .id(id.clone())
        .absolute()
        .right_0()
        .top_full()
        .mt(surface::css(5.))
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(TooltipColors::border())
        .bg(TooltipColors::background())
        .text_color(TooltipColors::foreground())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_left()
        .whitespace_nowrap()
        .child(text)
        .with_animation(
            id,
            // CSS 是 `transition:visibility 0s,opacity .3s linear`；gpui 的默认缓动即线性。
            Animation::new(Duration::from_millis(300)),
            |tip, delta| tip.opacity(delta),
        )
        .into_any_element()
}

/// 同皮肤，但**不带挂载动画**：由 `group_hover(group)` 驱动 `opacity 0→1`。
///
/// 用于拿不到可写 `App`／无法建立 hover 状态的位置（例如 `Popover::trigger_with` 的闭包只给
/// `&mut Window` 与 `&App`）。该形态能对齐源的皮肤、锚点与显隐条件，但源 CSS 的
/// `transition:opacity .3s linear` 淡入在本地不可达，调用点必须在审计里登记这一点。
pub(crate) fn attribute_tip_group(
    id: impl Into<ElementId> + Clone,
    text: impl Into<SharedString>,
    group: &'static str,
) -> AnyElement {
    let text: SharedString = text.into();
    div()
        .id(id)
        .absolute()
        .right_0()
        .top_full()
        .mt(surface::css(5.))
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(TooltipColors::border())
        .bg(TooltipColors::background())
        .text_color(TooltipColors::foreground())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_left()
        .whitespace_nowrap()
        .opacity(0.)
        .group_hover(group, |style| style.opacity(1.))
        .child(text)
        .into_any_element()
}
