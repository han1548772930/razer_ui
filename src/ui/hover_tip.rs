//! `.tooltip-razer` 悬停提示（源 `hu`/`Gu` 组件）。
//!
//! 源组件在 `showTooltip` 里 `setState({shouldRender:!0})` **立刻挂载**，随后只用一个
//! `setTimeout(..., 0)` 加 `.show`，由 `.tooltip-razer>.main{transition:opacity .1s linear}`
//! 完成 100ms 淡入；`hideTooltip` 立刻去掉 `.show`（100ms 淡出）并在另一个 0ms 定时器后卸载。
//! 也就是说它没有任何展示延迟，位置与皮肤完全由下面的 `.tooltip-razer` 规则决定：
//!
//! ```css
//! .tooltip-razer>.main{bottom:100%;display:flex;justify-content:center;left:50%;
//!   margin-bottom:5px;margin-left:-150px;opacity:0;pointer-events:none;position:absolute;
//!   transition:opacity .1s linear;width:300px}
//! .tooltip-razer.show>.main{opacity:1}
//! .tooltip-razer.bottom-left>.main,.tooltip-razer.bottom-right>.main,.tooltip-razer.bottom>.main{
//!   bottom:auto;margin-bottom:0;margin-top:5px;top:100%}
//! .tooltip-razer.bottom-left>.main{justify-content:flex-end;left:auto;margin-left:0;right:0}
//! .tooltip-razer.bottom-right>.main{justify-content:flex-start;left:0;margin-left:0}
//! .tooltip-razer.top>.main{bottom:100%;left:50%;margin-bottom:5px;margin-left:-150px;margin-top:0;top:auto}
//! .tooltip-razer>.main>.wrapper{background-color:#000;border:1px solid #5d5d5d;color:#ccc;
//!   display:inline-block;font-family:Roboto;font-size:14px;line-height:16px;padding:8px 10px;
//!   text-align:left;text-transform:none}
//! ```
//!
//! 调用方负责在目标元素（源里都是带 id 的包装元素，例如 3884/3871 的
//! `#icon-detection-wrapper` / `#icon-refreshing-wrapper`）上挂 hover 监听，并在悬停时把
//! [`source_hover_tip`] 插到该包装元素里。
use crate::ui::{surface, theme::TooltipColors};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

/// 源组件还带 `bottom-left-edge` / `bottom-right-edge` / `top-left` / `top-right` 等位置；
/// 这里保留当前源已经用到或即将用到的四种，尚未接入的位置不臆造页面。
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SourceTipPlacement {
    /// `.tooltip-razer.bottom-left`：贴目标下沿、右缘对齐。
    BottomLeft,
    /// `.tooltip-razer.bottom-right`：贴目标下沿、左缘对齐。
    BottomRight,
    /// `.tooltip-razer.bottom`：贴目标下沿、居中。
    Bottom,
    /// `.tooltip-razer.top`：贴目标上沿、居中。
    Top,
}

/// 悬停时立即挂载的 `.tooltip-razer` 提示：没有展示延迟，只有源里那 100ms 的淡入。
pub(crate) fn source_hover_tip(
    id: impl Into<ElementId> + Clone,
    text: impl Into<SharedString>,
    placement: SourceTipPlacement,
) -> AnyElement {
    let text: SharedString = text.into();
    source_hover_tip_element(id, placement, text)
}

/// 同上，但提示内容由调用方提供：源里 LED 数量那类提示会把数字单独包在
/// `<span style="color:#44d62c">` 中（3871/778 的
/// `getTextItem(bO.vml,{ledCount:'<span style="color:#44d62c">N</span>'})`）。
pub(crate) fn source_hover_tip_element(
    id: impl Into<ElementId> + Clone,
    placement: SourceTipPlacement,
    content: impl IntoElement,
) -> AnyElement {
    let below = matches!(
        placement,
        SourceTipPlacement::BottomLeft
            | SourceTipPlacement::BottomRight
            | SourceTipPlacement::Bottom
    );
    div()
        .id(id.clone())
        .absolute()
        .w(surface::css(300.))
        .flex()
        .when(placement == SourceTipPlacement::BottomLeft, |tip| {
            tip.justify_end().right_0()
        })
        .when(placement == SourceTipPlacement::BottomRight, |tip| {
            tip.justify_start().left_0()
        })
        .when(
            matches!(
                placement,
                SourceTipPlacement::Bottom | SourceTipPlacement::Top
            ),
            |tip| {
                tip.justify_center()
                    .left(relative(0.5))
                    .ml(surface::css(-150.))
            },
        )
        .when(below, |tip| tip.top_full().mt(surface::css(5.)))
        .when(placement == SourceTipPlacement::Top, |tip| {
            tip.bottom_full().mb(surface::css(5.))
        })
        .child(
            div()
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
                .child(content),
        )
        .with_animation(
            id,
            Animation::new(Duration::from_millis(100)),
            |tip, delta| tip.opacity(delta),
        )
        .into_any_element()
}
