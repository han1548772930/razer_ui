//! 各页面共用的展示组件。
//!
//! # 约定
//!
//! 一律使用 gpui-kit 的既有组件，**不手搓 div 拼控件**：
//!
//! | 用途 | 组件 |
//! |---|---|
//! | 卡片容器 | [`GroupBox`] |
//! | 布尔开关 | [`Switch`] |
//! | 数值可视化 | [`BarChart`] / [`LineChart`]（canvas 类渲染） |
//!
//! 手搓的按钮/开关样式既不规范，外观也和雷云真实界面不一致。
//!
//! # 为什么分「组件」和「渲染辅助函数」两类
//!
//! 依据 gpui-kit 《Coding Guides · Choose the right unit》：
//!
//! > Use a `RenderOnce`/`IntoElement` component when all inputs can be supplied
//! > by the caller and the element does not need to retain application state
//! > between frames.
//!
//! 因此凡是**只依赖入参**的展示件都是 [`RenderOnce`] 组件，带 `::new()`：
//! [`PageHeader`]、[`SettingRow`]、[`EmptyState`]、[`ColorSwatch`]、[`PageLayout`]。
//!
//! 只有 [`toggle_button`] 与 [`stepper_row`] 仍是自由函数——它们的回调要拿到
//! `Context<AppShell>`（`Switch::on_change` 只给 `&mut App`），这属于
//! **应用自身的组合策略**，不是可复用组件的契约。见各自的文档注释。

use gpui_kit::component::{
    button::*, chart::BarChart, group_box::GroupBox, popover::Popover, *,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;

/// 雷云页面的**几何契约**常量。
///
/// 这些不是配色，而是复刻对象本身的固定尺寸（卡片 600px、产品图区 250px…），
/// 逐条引自设备模块 CSS，**颜色一律走 `cx.theme()`**，不在此处定义。
/// 全量清单见 [`docs/screens/00-visual-system.md`](../../docs/screens/00-visual-system.md)。
pub mod geometry {
    /// `.main-container` / `.body-wrapper` 的最小宽度
    pub const SHELL_MIN_WIDTH: f32 = 600.0;
    /// `.body-widgets { max-width:1240px }`
    pub const BODY_MAX_WIDTH: f32 = 1240.0;
    /// `.widget { min-width:600px; max-width:600px }`
    pub const WIDGET_WIDTH: f32 = 600.0;
    /// `.widget-prod { height:250px }`
    pub const PRODUCT_BANNER_HEIGHT: f32 = 250.0;
    /// `.widget-prod { min-width:1024px }`
    pub const PRODUCT_BANNER_MIN_WIDTH: f32 = 1024.0;
    /// `.widget-prod { max-width:1220px }`
    pub const PRODUCT_BANNER_MAX_WIDTH: f32 = 1220.0;
    /// `.widget { padding:30px 40px }`
    pub const WIDGET_PADDING_Y: f32 = 30.0;
    /// `.widget { padding:30px 40px }`
    pub const WIDGET_PADDING_X: f32 = 40.0;
    /// `.body-wrapper { padding:10px 20px 20px }`
    pub const BODY_PADDING_TOP: f32 = 10.0;
    /// `.body-wrapper { padding:10px 20px 20px }`
    pub const BODY_PADDING_SIDE: f32 = 20.0;
    /// `.widget { margin:10px auto }`
    pub const WIDGET_MARGIN_Y: f32 = 10.0;

    /// `.body-widgets .widget { border-radius:5px }`
    ///
    /// 不直接用 `theme.radius`：后者是 3px，是全站出现最多的圆角（89 处），
    /// 但 `.widget` 这个表面用的是 5px。
    pub const WIDGET_RADIUS: f32 = 5.0;

    /// `.thx-btn { border-radius:3px }`
    pub const BUTTON_RADIUS: f32 = 3.0;

    /// `.thx-btn.fit, .thx-btn.inline { height:27px; font-size:12px; line-height:12px }`
    pub const BUTTON_SM_HEIGHT: f32 = 27.0;

    /// `.thx-btn.fit, .thx-btn.inline { padding:7px 5px 6px }` —— 取横向 5px。
    /// 纵向由上面的固定高度吸收。
    pub const BUTTON_SM_PADDING_X: f32 = 5.0;

    // ---- 开关 `.switch` ----

    /// `.switch { width:32px }`
    pub const SWITCH_W: f32 = 32.0;
    /// `.switch { height:18px }`
    pub const SWITCH_H: f32 = 18.0;
    /// `.switch { padding:2px }`
    pub const SWITCH_PAD: f32 = 2.0;
    /// `.switch { border-radius:16px }` —— 胶囊。
    pub const SWITCH_RADIUS: f32 = 16.0;
    /// `.switch .handle { width:14px; height:14px }`
    pub const SWITCH_THUMB: f32 = 14.0;
    /// `.switch .handle { border-radius:7px }` —— 圆形。
    pub const SWITCH_THUMB_RADIUS: f32 = 7.0;
    /// `.switch .handle { left:1px }`（关态）
    pub const SWITCH_THUMB_OFF: f32 = 1.0;
    /// `.switch.on .handle { left:15px }`（开态）。
    ///
    /// 自洽性：轨道 32 − 内距 2×2 − 滑块 14 = **14**，正好等于 `15 − 1`。
    /// 也就是说真实值用的是 `box-sizing:border-box`（GPUI 的边框同样是内绘）。
    pub const SWITCH_THUMB_ON: f32 = 15.0;
    /// `.switch { border:1px solid #0000004d }` —— 30% 黑。
    /// 主题里没有「半透明描边」档位，按本模块约定用具名常量。
    pub const SWITCH_BORDER: u32 = 0x4D00_0000;
    /// `.widget .title .switch { left:10px }` —— 标签与开关之间的间距。
    pub const SWITCH_LABEL_GAP: f32 = 10.0;

    // ---- 下拉 `.s3-dropdown` / `.s3-options` ----

    /// `.s3-dropdown { height:27px }`
    pub const DROPDOWN_H: f32 = 27.0;
    /// `.s3-dropdown { padding:4px 5px }` —— 取横向 5px。
    pub const DROPDOWN_PAD_X: f32 = 5.0;
    /// 下拉的横向宽度。真实值是 `width:100%`（跟随父容器），
    /// 这里给一个固定值，使右对齐的控件列宽度一致。
    pub const DROPDOWN_W: f32 = 220.0;
    /// `.s3-dropdown { border:1px solid #515151 }`、`.s3-options.expand { border:1px solid #515151 }`
    ///
    /// 主题里没有这个档位（通用边框是 `#5d5d5d`），因此用具名常量。
    pub const DROPDOWN_BORDER: u32 = 0x0051_5151;
    /// `.s3-options { background-color:#000 }` —— 列表底色是纯黑，不是 `popover` 的 `#111`。
    pub const DROPDOWN_LIST_BG: u32 = 0x0000_0000;
    /// `.s3-options.expand { max-height:180px }`
    pub const DROPDOWN_LIST_MAX_H: f32 = 180.0;
    /// `.s3-options .option { height:25px; min-height:25px; padding:4px }`
    pub const DROPDOWN_OPTION_H: f32 = 25.0;
    /// `.s3-options .option { padding:4px }`
    pub const DROPDOWN_OPTION_PAD: f32 = 4.0;
    /// `.s3-options .option:hover { background-color:#ffffff1a }` —— 10% 白。
    pub const DROPDOWN_OPTION_HOVER: u32 = 0x1AFF_FFFF;
    // 注：`.s3-options .option.inactive { color:#cccccc4d }`（30% 的 `#ccc`）
    // 表示「当前不可选」的选项。本实现的下拉还没有这个概念——所有选项都可选，
    // 因此不为它留一个无人使用的常量。等真的需要禁用某些选项时再加。

    // ---- 滑块 `.slider-container` ----

    /// `.slider-container { height:64px }`（带刻度提示）；
    /// `.slider-container.no-tip { height:36px }`
    pub const SLIDER_H: f32 = 64.0;
    /// `.has-slider .slider-container { margin-left:30px; width:490px }`
    pub const SLIDER_W: f32 = 490.0;
    /// `.slider-container .track { bottom:25px; height:6px }`
    pub const SLIDER_TRACK_H: f32 = 6.0;
    /// `.slider-container .track { bottom:25px }`
    pub const SLIDER_TRACK_BOTTOM: f32 = 25.0;
    /// `.slider-container .track { border-radius:3px }`
    pub const SLIDER_TRACK_RADIUS: f32 = 3.0;
    /// `.slider::-webkit-slider-thumb { width:16px; height:16px }`
    pub const SLIDER_THUMB: f32 = 16.0;
    /// `.slider::-webkit-slider-thumb { border-radius:8px }` —— 圆形。
    pub const SLIDER_THUMB_RADIUS: f32 = 8.0;
    /// `.slider-container .foot { bottom:-2px }`
    pub const SLIDER_FOOT_BOTTOM: f32 = -2.0;
    /// `.slider-container { opacity:.3 }`（不可用时）
    pub const SLIDER_DISABLED_OPACITY: f32 = 0.3;
    /// `.slider-container .track { background:#44d62c4d }` —— 30% 的雷蛇绿。
    pub const SLIDER_TRACK_BG: u32 = 0x44D6_2C4D;
    /// `.slider-container .left { background:#44d62c }` —— 已填充部分满绿。
    pub const SLIDER_FILL_BG: u32 = 0x0044_D62C;
    /// `.slider::-webkit-slider-thumb { background:#44d62c }`
    pub const SLIDER_THUMB_BG: u32 = 0x0044_D62C;

    // ---- 字阶 ----
    //
    // 雷云用的是**固定档位**的字阶，不是比例派生。由
    // `.ref/tools/font-size-audit.js` 统计全部 CSS 得到（1821 条声明、28 个取值）：
    //
    // | 字号 | 条数 | 代表选择器 |
    // |---|---|---|
    // | 14px | 1124 | `.desc-text`、`.thx-btn` 默认（主字阶） |
    // | 12px | 371 | `.thx-btn.sm`/`.fit`/`.inline`、`.navs-wrapper`、`.slider-container .title-more` |
    // | 16px | 121 | `body,html`、`.widget .titleRow .title` |
    // | 18px | 25 | `.main-setting .widget .title` |
    // | 20px | 27 | `.main-title` |
    //
    // gpui-kit 的字号是从 `theme.font_size` 按 `×0.875` / `×0.75` 派生的
    // （`gpui-component/src/sizing.rs:239`、`accordion.rs:284`），
    // **无法同时**得到「16px 正文 + 12px 小按钮」——那需要 font_size≈13.7px，
    // 而正文又必须是 16px。所以基字号取真实的 16px，需要固定档位处用下面的常量。

    /// `.badge`、`.slider-container.no-bordered .thumb-tag`
    pub const TEXT_10: f32 = 10.0;
    /// `.thx-btn.sm` / `.thx-btn.fit` / `.navs-wrapper` / `.slider-container .title-more`
    pub const TEXT_12: f32 = 12.0;
    /// 次级说明（`.sw-option__desc` 等）
    pub const TEXT_13: f32 = 13.0;
    /// `.desc-text`、`.body-text`、`.widget` 正文 —— 全站主字阶（1124 条）
    pub const TEXT_14: f32 = 14.0;
    /// `body,html`、`.widget .titleRow .title` —— 卡片标题
    pub const TEXT_16: f32 = 16.0;
    /// `.main-setting .widget .title` —— 设置页卡片标题
    pub const TEXT_18: f32 = 18.0;
    /// `.main-title`、`.snap-tap-add-button`
    pub const TEXT_20: f32 = 20.0;
}

// ---------------------------------------------------------------------------
// 设备页的真实布局
// ---------------------------------------------------------------------------
//
// 这一段不是自拟的，数值逐条来自设备模块的 CSS（`.ref/devices/*/static/css/`）：
//
// ```css
// .main-container { display:flex; flex-direction:column; height:100% }
// .body-wrapper   { flex:1 1; height:100%; padding:10px 20px 20px }
// .body-widgets   { flex-direction:row; flex-wrap:wrap; justify-content:center;
//                   margin:auto; max-width:1240px }
// .widget-col     { flex-direction:column; height:fit-content; width:600px }
// .body-widgets .widget { flex:0 0 auto; min-width:600px; max-width:600px;
//                         margin:10px auto; padding:30px 40px; position:relative }
// .widget-prod    { height:250px; margin:10px auto;
//                   min-width:1024px; max-width:1220px; width:100% }
// .widget-prod img { left:50%; position:absolute; top:50% }
// ```
//
// 结论：设备页**不是**「一列全宽卡片」，而是
// **顶部一条 250px 高的产品图区 + 下方固定 600px 宽的卡片两列自动换行**。

/// 设备页主体容器，对应雷云的 `.body-widgets`。
///
/// ```css
/// .body-widgets { flex-direction:row; flex-wrap:wrap; justify-content:center;
///                 margin:auto; max-width:1240px }
/// ```
///
/// 横向排列 + 自动换行 + 居中，最大宽度 1240px；
/// 子项是 [`widget_card`]，固定 600px 宽，因此**一行正好两个**。
///
/// 内边距**不在这里**：`padding:10px 20px 20px` 属于 `.body-wrapper`，
/// 由外壳的滚动容器承担（见 `src/app.rs` 的 `Render for AppShell`）。
/// 此前重复加过一次，已纠正。
pub fn body_widgets() -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .w_full()
        .max_w(px(geometry::BODY_MAX_WIDTH))
        .mx_auto()
}

/// 一个 widget 卡片，对应雷云的 `.body-widgets .widget` 的**外槽位**。
///
/// ```css
/// .body-widgets .widget { flex:0 0 auto; height:auto; margin:10px auto;
///                         max-width:600px; min-width:600px;
///                         padding:30px 40px; position:relative;
///                         background-color:#111; border-radius:5px }
/// ```
///
/// 固定 600px 宽、`margin:10px auto`；内边距与底色由 [`card`] 提供。
/// 因为固定 600px 且容器最大 1240px，**一行正好放两个**，超出自动换行。
///
/// 注意：这里**不收 `cx`**。收了会导致
/// `widget_card(cx, card(cx).child(|cx| ...))` 出现
/// 「先不可变借用、再在闭包里可变借用」的冲突。
pub fn widget_card(content: GroupBox) -> AnyElement {
    widget_slot(content.into_any_element())
}

/// 把**任意**已构造好的元素放进一个 600px widget 槽位。
///
/// [`widget_card`] 要求内容是 `GroupBox`；已有页面把「卡片 + 内容」整链
/// 构造成 `AnyElement` 了，这个变体让它们不必重写就能进入真实布局。
/// 两者共用同一份外槽位规格，避免风格漂移。
pub fn widget_slot(content: AnyElement) -> AnyElement {
    div()
        // `.widget { flex:0 0 auto }`
        .flex_grow(0.)
        .flex_shrink_0()
        .w(px(geometry::WIDGET_WIDTH))
        .min_w(px(geometry::WIDGET_WIDTH))
        .max_w(px(geometry::WIDGET_WIDTH))
        // `.widget { margin:10px auto }`
        .my(px(geometry::WIDGET_MARGIN_Y))
        .mx_auto()
        // `.widget { position:relative }` —— 卡片内的 `.tip` / 角标靠它定位。
        .relative()
        .child(content)
        .into_any_element()
}

/// 把 `[r, g, b]` 转成 `rgb()` 需要的 u32。
///
/// 纯函数，不涉及元素构造，因此保留为自由函数。
pub fn rgb_u32(color: [u8; 3]) -> u32 {
    ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32)
}

/// 统一的卡片容器，对应雷云的 `.widget` 表面。
///
/// 用 gpui-kit 的官方分组容器 [`GroupBox`]；它实现了 `ParentElement`，
/// 因此调用方的 `.child(...)` 用法完全不变。
///
/// **不收 `cx`**：`GroupBox` 自身不带参数，此前那个 `_cx: &App` 从未被使用，
/// 属于无用的伪依赖，已删除。
pub fn card() -> GroupBox {
    // `.body-widgets .widget { padding:30px 40px; border-radius:5px }`
    //
    // `GroupBox` 自带 `p_4()` 与 `rounded(theme.radius)`（=3px）。这里显式覆盖成
    // 雷云的密度与圆角，是**应用层的选择**，符合 gpui-kit
    // 《Design Guides · Components and composition》：
    // > GPUI Component supplies coherent defaults, while the application owns
    // > composition and product semantics.
    //
    // 圆角用 [`geometry::WIDGET_RADIUS`] 而不是 `theme.radius`：`theme.radius`
    // 的 3px 是全站出现最多的圆角（89 处），但 `.widget` 用的是 5px。
    GroupBox::new()
        .py(px(geometry::WIDGET_PADDING_Y))
        .px(px(geometry::WIDGET_PADDING_X))
        .rounded(px(geometry::WIDGET_RADIUS))
        // `body,html` 是 16px，但 `.widget` 把正文压回 14px
        // （`.widget { font-size:14px }`，全站 1124 条声明都是这个值）。
        .text_size(px(geometry::TEXT_14))
}

/// 卡片标题，对应 `.widget .titleRow .title`。
///
/// ```css
/// .widget .titleRow .title { font-size:16px }
/// .main-setting .widget .title { font-size:18px }   /* 设置页，见 setting.rs */
/// ```
///
/// 之前卡片标题只是 `div().font_bold()`，会继承 `.widget` 的 14px，
/// **比真实的 16px 小两号**。这里把字号收进一个组件，
/// 免得 92 个调用点各写一遍。
pub fn card_title(text: impl Into<SharedString>) -> AnyElement {
    div()
        .font_bold()
        .text_size(px(geometry::TEXT_16))
        .child(text.into())
        .into_any_element()
}

/// 应用级按钮，对应雷云的 `.thx-btn` / `.thx-btn.fit`。
///
/// ```css
/// .thx-btn { display:inline-block; padding:.5rem 1.5rem; text-align:center;
///            text-transform:uppercase; transition:opacity .3s;
///            background-color:#44d62c; border-radius:3px; color:#000 }
/// .thx-btn.fit, .thx-btn.inline { display:block!important; font-size:12px;
///            height:27px; line-height:12px; margin:auto;
///            border:1px solid #0000004d }
/// .thx-btn.fit { max-width:fit-content; padding:7px 5px 6px }
/// .thx-btn.secondary { background-color:#707070; color:#fff }
/// .thx-btn.disabled, .thx-btn.disabled:hover { cursor:default; opacity:.3 }
/// ```
///
/// # 为什么需要它
///
/// gpui-kit 的 `Button::small()` 给的是 `h_6()`（24px）与 `px_2()`（8px），
/// 而雷云的行内按钮是 **27px 高、`padding:7px 5px 6px`、字号 12px、圆角 3px**。
/// 按 gpui-kit 《Design Guides · Components and composition》
/// 「the application owns composition and product semantics」，
/// 这里把产品规格封成一个入口，避免 40 多处调用点各写一遍、
/// 也避免将来改规格要改 40 处。
///
/// 取 `.fit` 而不是固定 90px 的 `.sm`：标签是中文，长度不一，
/// `.fit` 的 `max-width:fit-content` 才是对应行为。
pub fn btn(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    let label: SharedString = label.into();
    Button::new(id)
        .small()
        .h(px(geometry::BUTTON_SM_HEIGHT))
        .px(px(geometry::BUTTON_SM_PADDING_X))
        .rounded(px(geometry::BUTTON_RADIUS))
        // `.thx-btn.sm { font-size:12px; height:27px; line-height:11px }`
        //
        // 必须显式指定：gpui 的 `Button::small()` 走
        // `button_text_size(Size::Small)` = `text_sm()`（`sizing.rs:330`），
        // 而 `text_sm` 由 `theme.font_size × 0.875` 派生。基字号是真实的
        // 16px，`text_sm` 会算成 14px，比真实的 12px 大两号。
        .text_size(px(geometry::TEXT_12))
        // `.thx-btn { text-transform:uppercase }`。中文无大小写，等于原样。
        .label(label.to_uppercase())
}

/// 页头：大标题 + 副标题。
///
/// 展示件，输入全部来自调用方，因此是 [`RenderOnce`] 组件。
///
/// [`PageHeader::new`]: PageHeader::new
#[derive(IntoElement)]
pub struct PageHeader {
    title: SharedString,
    subtitle: SharedString,
}

impl PageHeader {
    /// `title` 是分区名，`subtitle` 说明当前设备与保存行为。
    pub fn new(title: impl Into<SharedString>, subtitle: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
        }
    }
}

impl RenderOnce for PageHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        v_flex()
            .gap_1()
            .child(div().text_xl().font_bold().child(self.title))
            .child(div().text_sm().child(self.subtitle))
    }
}

/// 「设置项 —— 值」一行。
///
/// 这是设备页里出现最多的重复模式（各页合计 60 余处）。
///
/// # 为什么没有用 gpui-kit 的 `SettingItem`
///
/// 规范要求「先用标准组件，不要用 div 复刻」，因此这里确实调研过
/// `gpui_kit::component::setting` 组件族，结论是**结构上不可用**：
///
/// | 类型 | `IntoElement` | `RenderOnce` |
/// |---|---|---|
/// | `SettingItem` | ❌ | ❌ |
/// | `SettingGroup` | ❌ | ❌ |
/// | `SettingPage` | ❌ | ❌ |
/// | `Settings` | 经 `RenderOnce` | ✅ |
///
/// 即整族**只能从根 `Settings` 渲染**，而 `Settings` 自带一整套侧边栏 +
/// 分页导航（`Settings::page` / `sidebar_width` / `default_selected_index`）。
/// 用它当根就会**替换掉雷云的设备侧栏 + 标签栏**，而这次替代的明确前提是
/// 「UI 不能随便替换、布局要与雷云等价」。
///
/// 因此这里保留一个应用层组件。这正是 《Design Guides · Components and
/// composition》允许的：
///
/// > Keep a repeated pattern consistent across the product. Wrap it in an
/// > application component when it carries domain language or policy.
///
/// 每个设备页的卡片里是「设置项 + 读数 + 控件」的混合内容，并非
/// `Settings` 那种「整页表单」形态，所以这一层由应用自己拥有是合适的。
#[derive(IntoElement)]
pub struct SettingRow {
    label: SharedString,
    value: SharedString,
}

impl SettingRow {
    /// `label` 是设置项名，`value` 是当前值（只读展示）。
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .gap_3()
            .text_sm()
            .child(div().child(self.label))
            .child(div().font_bold().child(self.value))
    }
}

/// 空状态提示卡片。
///
/// 输入只有一句说明，因此是 [`RenderOnce`] 组件；不再需要 `cx`。
#[derive(IntoElement)]
pub struct EmptyState {
    text: SharedString,
}

impl EmptyState {
    /// `text` 说明为什么这里是空的（不要只说「无数据」，要说清原因）。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        card().child(div().text_sm().child(self.text))
    }
}

/// 颜色小方块（灯光页的色板用）。
///
/// 颜色由调用方给出，组件本身不持有状态，因此是 [`RenderOnce`] 组件；
/// 边框色取自主题，不在调用点写颜色。
#[derive(IntoElement)]
pub struct ColorSwatch {
    color: [u8; 3],
}

impl ColorSwatch {
    /// `color` 是 `[r, g, b]`。
    pub fn new(color: [u8; 3]) -> Self {
        Self { color }
    }
}

impl RenderOnce for ColorSwatch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .size_4()
            .flex_shrink_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(rgb(rgb_u32(self.color)))
    }
}

/// 设备页的统一外壳：页头 + 产品图区 + 600px 卡片区。
///
/// 对应雷云真实结构（[`docs/screens/00-visual-system.md`](../../docs/screens/00-visual-system.md) §2.0）：
///
/// ```text
/// .body-wrapper
/// ├─ 页头（本实现为可读性加的标题行）
/// ├─ .widget-prod      产品图区，高 250px、宽 1024–1220px
/// └─ .body-widgets     600px 卡片两列换行，最大宽 1240px
/// ```
///
/// 各页面只交出分区，不各自重复搭外壳——这样「卡片固定 600px、一行两个」
/// 的布局契约只有一处实现，页面之间不会漂移。
#[derive(IntoElement)]
pub struct PageLayout {
    title: SharedString,
    device_name: SharedString,
    subtitle: Option<SharedString>,
    widgets: Vec<AnyElement>,
}

impl PageLayout {
    /// `title` 是标签页名；`device_name` 显示在副标题与产品图区里。
    ///
    /// 副标题默认是「{设备} · 更改会立即保存」；需要额外信息（例如
    /// 「{设备} · 192 个可自定义输入点 · 更改会立即保存」）时用 [`subtitle`]。
    ///
    /// [`subtitle`]: PageLayout::subtitle
    pub fn new(title: impl Into<SharedString>, device_name: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            device_name: device_name.into(),
            subtitle: None,
            widgets: Vec::new(),
        }
    }

    /// 覆盖默认副标题。仍应保留「更改会立即保存」这类状态说明。
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// 追加一个分区卡片。builder 返回 `Self`，可链式调用。
    pub fn widget(mut self, widget: impl IntoElement) -> Self {
        self.widgets.push(widget.into_any_element());
        self
    }

    /// 批量追加分区（例如已经把各区域收进 `Vec` 的页面）。
    pub fn widgets(mut self, widgets: impl IntoIterator<Item = impl IntoElement>) -> Self {
        for widget in widgets {
            self.widgets.push(widget.into_any_element());
        }
        self
    }
}

impl RenderOnce for PageLayout {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let device_name = self.device_name.clone();
        let subtitle = self
            .subtitle
            .unwrap_or_else(|| format!("{device_name} · 更改会立即保存").into());

        v_flex()
            .size_full()
            .min_w(px(geometry::SHELL_MIN_WIDTH))
            .gap_2()
            .child(PageHeader::new(self.title, subtitle))
            .child(ProductBanner::new(device_name))
            .child(body_widgets().children(self.widgets))
    }
}

/// 顶部产品图区，对应雷云的 `.widget-prod`。
///
/// 真实布局是 250px 高、1024–1220px 宽，设备图片**绝对居中**
/// （`.widget-prod img { left:50%; top:50% }`）。
/// 本实现没有雷云素材（版权归 Razer），因此用同尺寸的占位框，
/// 但**保持尺寸与位置一致**。
///
/// 文字色取自 `cx.theme().muted_foreground`——占位说明属于次要信息，
/// 按 《Design Guides · Color and themes》用语义档位，不在调用点写颜色。
#[derive(IntoElement)]
pub struct ProductBanner {
    device_name: SharedString,
}

impl ProductBanner {
    /// `device_name` 显示在占位框中央，说明这里本该是产品图。
    pub fn new(device_name: impl Into<SharedString>) -> Self {
        Self {
            device_name: device_name.into(),
        }
    }
}

impl RenderOnce for ProductBanner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        div()
            .h(px(geometry::PRODUCT_BANNER_HEIGHT))
            .w_full()
            .min_w(px(geometry::PRODUCT_BANNER_MIN_WIDTH))
            .max_w(px(geometry::PRODUCT_BANNER_MAX_WIDTH))
            .mx_auto()
            .my(px(geometry::WIDGET_MARGIN_Y))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!(
                        "{} · 产品图位置（250px 高，图片居中）",
                        self.device_name
                    )),
            )
    }
}

/// 布尔开关，对应雷云的 `.switch`。
///
/// ```css
/// .switch { display:inline-block; position:relative; width:32px; height:18px;
///           padding:2px; background-color:#707070; border:1px solid #0000004d;
///           border-radius:16px; transition:background-color .3s }
/// .switch:hover { opacity:.7 }
/// .switch.on { background-color:#44d62c }
/// .switch .handle { position:absolute; left:1px; top:1px;
///                   width:14px; height:14px; background-color:#111;
///                   border-radius:7px; transition:left .2s }
/// .switch.on .handle { left:15px }
/// .switch.disabled { opacity:.3 }
/// ```
///
/// # 为什么不用 gpui-kit 的 [`Switch`]
///
/// 1. **几何对不上**：库按 `Size` 写死，`Small` = 28×16、默认 = 36×20，
///    而真实是 **32×18**，滑块 14（库是 12 / 16）。
/// 2. **胶囊改不了**：轨道圆角取自 `cx.theme().radius`
///    （`if theme.radius >= px(4.) { 全圆角 } else { theme.radius }`），
///    而 `impl Styled for Switch` 的 `.rounded()` 作用在**外层 wrapper** 上，
///    碰不到轨道。全站 `radius` 是 3px，改成 ≥4px 会连带改掉整个应用。
/// 3. **滑块颜色反了**：主题里 `switch_thumb` 此前被我设成 `#ccc`（浅色），
///    真实是 **`#111`**——滑块是深色，不是白色。
/// 4. **关态轨道也反了**：此前 `#333`，真实是 **`#707070`**。
///
/// 因此按《Design Guides · Components and composition》
/// 「the application owns composition and product semantics」做成应用组件；
/// 颜色走主题令牌（`theme.switch` / `theme.switch_thumb` / `theme.primary`），
/// 几何走 [`geometry`] 里带出处注释的常量。
pub fn toggle_button(
    id: &'static str,
    label: &'static str,
    on: bool,
    cx: &mut Context<AppShell>,
    handler: impl Fn(&mut AppShell, &mut Context<AppShell>) + 'static,
) -> AnyElement {
    let entity = cx.entity();
    // `.switch { background-color:#707070 }` / `.switch.on { background-color:#44d62c }`
    let track = if on {
        cx.theme().primary
    } else {
        cx.theme().switch
    };
    // `.switch .handle { background-color:#111 }`
    let thumb = cx.theme().switch_thumb;
    // `.switch { border:1px solid #0000004d }`
    let border = rgb(geometry::SWITCH_BORDER);
    let travel = if on {
        geometry::SWITCH_THUMB_ON
    } else {
        geometry::SWITCH_THUMB_OFF
    };

    h_flex()
        .gap(px(geometry::SWITCH_LABEL_GAP))
        .items_center()
        .child(div().text_sm().child(label))
        .child(
            div()
                .id(id)
                .relative()
                .flex_shrink_0()
                .w(px(geometry::SWITCH_W))
                .h(px(geometry::SWITCH_H))
                .p(px(geometry::SWITCH_PAD))
                .rounded(px(geometry::SWITCH_RADIUS))
                .border_1()
                .border_color(border)
                .bg(track)
                .cursor_pointer()
                // `.switch:hover { opacity:.7 }`
                .hover(|this| this.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(geometry::SWITCH_THUMB_OFF))
                        .left(px(travel))
                        .w(px(geometry::SWITCH_THUMB))
                        .h(px(geometry::SWITCH_THUMB))
                        .rounded(px(geometry::SWITCH_THUMB_RADIUS))
                        .bg(thumb),
                )
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| handler(this, cx));
                }),
        )
        .into_any_element()
}

/// 「标签 + 下拉」行，对应雷云的 `.s3-dropdown` + `.s3-options`。
///
/// ```css
/// .s3-dropdown { font-size:14px; height:27px; max-height:27px; padding:4px 5px;
///                text-transform:none; width:100%; position:relative;
///                background-color:#0000; border:1px solid #515151; color:#ccc }
/// .s3-dropdown:hover, .s3-dropdown.expand { border:1px solid #44d62c }
/// .s3-dropdown.disabled { pointer-events:none; opacity:.3 }
/// .s3-dropdown.placeholder { color:#666 }
/// .s3-options { position:absolute; left:0; margin-top:1px; flex-direction:column;
///               overflow-y:auto; min-width:100%; width:fit-content;
///               background-color:#000; border:none }
/// .s3-options.expand { height:auto; max-height:180px; border:1px solid #515151 }
/// .s3-options .option { font-size:14px; height:25px; min-height:25px; padding:4px;
///                       width:100%; overflow:hidden; white-space:nowrap }
/// .s3-options .option:hover { background-color:#ffffff1a }
/// .s3-options .option.selected, .s3-options .option:active { color:#44d62c }
/// .s3-options .option.inactive { color:#cccccc4d }
/// ```
///
/// 弹层的定位、点击外部关闭、Esc 关闭都由 gpui-kit 的 [`Popover`] 负责，
/// 这里只负责外观与选中回调。因此不需要自绘 `deferred` / `anchored`。
///
/// # 为什么不用库的 `Select`
///
/// `Select` 要求为每个下拉实现一个 `SearchableListDelegate` 泛型，
/// 且它的外观取自 `theme.input`（`#111` 边框）与 `theme.radius`（3px），
/// **没有** `.s3-dropdown` 的三个特征：透明底、`#515151` 边框、悬停/展开变绿边。
/// `PopupMenu` 则是 **Action 驱动**的（`menu(label, Box<dyn Action>)`），
/// 而这里要的是「从一组字符串里选一个」，Actions 是类型、无法在运行时生成。
/// # 受控开合
///
/// `open` 由调用方从 [`AppShell::open_select`] 传入。之所以受控：选中后要收起
/// 列表，而选项点击回调的上下文是 `&mut App`，拿不到
/// `Context<PopoverState>`（`PopoverState::dismiss` 要后者）。
/// 把开合状态放在应用侧后，选项回调只需 `&mut App`。
///
/// `id` 是 `SharedString` 而不是 `&'static str`：同一页可能有多个同类型下拉
/// （例如 Snap Tap 的每一组都有「录入模式」），必须能拼出互不相同的元素 id。
pub fn select_row(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    value: String,
    options: &'static [&'static str],
    open: bool,
    cx: &mut Context<AppShell>,
    pick: impl Fn(&mut AppShell, usize, &mut Context<AppShell>) + 'static,
) -> AnyElement {
    use std::rc::Rc;

    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let entity = cx.entity();
    // `.s3-options` 的内容闭包是 `Fn`，会被多次调用，因此回调共享一份 `Rc`。
    let pick: Rc<dyn Fn(&mut AppShell, usize, &mut Context<AppShell>)> = Rc::new(pick);

    let face_border = rgb(geometry::DROPDOWN_BORDER);
    let face_hover = cx.theme().primary;
    let list_bg = rgb(geometry::DROPDOWN_LIST_BG);
    let option_hover = rgb(geometry::DROPDOWN_OPTION_HOVER);
    let selected_fg = cx.theme().primary;
    let value_fg = cx.theme().foreground;

    // `.s3-dropdown { color:#ccc }`
    let face_value = value.clone();

    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().text_sm().child(label))
        .child(
            Popover::new(ElementId::Name(id.clone()))
                .anchor(Anchor::TopRight)
                // 受控开合：状态在 `AppShell::open_select`，这样选项点击时
                // 只需 `&mut App` 就能收起（`PopoverState::dismiss` 要
                // `Context<PopoverState>`，在选项回调里拿不到）。
                .open(open)
                .on_open_change({
                    let open_id = id.clone();
                    let entity = entity.clone();
                    move |is_open, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.open_select = if *is_open { Some(open_id.clone()) } else { None };
                            cx.notify();
                        });
                    }
                })
                .trigger(
                    // `.s3-dropdown`：透明底 + `#515151` 边框 + 悬停变绿 + 直角。
                    //
                    // 触发元素必须是 `Button`——`Popover::trigger` 要求
                    // `Selectable`，而 `Stateful<Div>` 没有实现它。
                    // 所以用 `Ghost` 变体（无填充）再逐项覆盖成下拉外观。
                    Button::new(ElementId::Name(id.clone()))
                        .with_variant(ButtonVariant::Ghost)
                        .label(face_value)
                        .icon(IconName::ChevronDown)
                        .w(px(geometry::DROPDOWN_W))
                        .h(px(geometry::DROPDOWN_H))
                        .px(px(geometry::DROPDOWN_PAD_X))
                        .flex_shrink_0()
                        .border_1()
                        .border_color(face_border)
                        .text_color(value_fg)
                        .text_sm()
                        .cursor_pointer()
                        .hover(move |this| this.border_color(face_hover)),
                )
                .content({
                    let entity = entity.clone();
                    move |_state, _window, _cx| {
                        let entity = entity.clone();
                        let pick = pick.clone();
                    v_flex()
                        // `.s3-options { background-color:#000 }` + `.expand` 的边框
                        .id(ElementId::Name(
                            format!("s3-options-{id}").into(),
                        ))
                        .bg(list_bg)
                        .border_1()
                        .border_color(face_border)
                        .max_h(px(geometry::DROPDOWN_LIST_MAX_H))
                        .overflow_y_scroll()
                        .children(options.iter().enumerate().map(|(index, option)| {
                            let entity = entity.clone();
                            let pick = pick.clone();
                            let selected = *option == value.as_str();
                            div()
                                .id(ElementId::Name(
                                    format!("s3-option-{id}-{index}").into(),
                                ))
                                .w_full()
                                .h(px(geometry::DROPDOWN_OPTION_H))
                                .px(px(geometry::DROPDOWN_OPTION_PAD))
                                .text_sm()
                                .truncate()
                                // `.option.selected { color:#44d62c }`
                                .when(selected, |this| this.text_color(selected_fg))
                                .cursor_pointer()
                                // `.option:hover { background-color:#ffffff1a }`
                                .hover(move |this| this.bg(option_hover))
                                .child(*option)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        pick(this, index, cx);
                                        // 选中后收起列表（受控开合）。
                                        this.open_select = None;
                                        cx.notify();
                                    });
                                })
                        }))
                    }
                }),
        )
        .into_any_element()
}

/// 「标签 + 滑块」行，对应雷云的 `.slider-container`。
///
/// ```css
/// .slider-container { position:relative; height:64px; opacity:.3;
///                     pointer-events:none; transition:opacity .3s }
/// .slider-container.on { opacity:1; pointer-events:auto }
/// .slider-container.no-tip { height:36px }
/// .has-slider .slider-container { margin-left:30px; width:490px }
/// .slider-container .track { position:absolute; bottom:25px; height:6px; width:100%;
///                            background:#44d62c4d; border-radius:3px; z-index:1 }
/// .slider-container .left, .slider-container .right {
///                            position:absolute; bottom:25px; height:6px; z-index:2;
///                            background:#44d62c; border-radius:3px }
/// .slider-container .foot { position:absolute; bottom:-2px; text-transform:uppercase }
/// .slider-container .title-more { position:absolute; bottom:-25px; font-size:12px;
///                            line-height:14px; color:#999; text-transform:uppercase }
/// .slider::-webkit-slider-thumb { background:#44d62c; border-radius:8px;
///                                 width:16px; height:16px }
/// .slider-container.on .slider::-webkit-slider-thumb:hover
///                            { background:#5d5d5d; border:2px solid #44d62c }
/// .slider-container.on .slider::-webkit-slider-thumb:active
///                            { background:#383838; border:2px solid #44d62c }
/// ```
///
/// # 为什么不用 gpui-kit 的 `Slider`
///
/// 1. **没有变更回调**。`Slider` 只暴露 `new/horizontal/vertical/disabled/reverse`，
///    值只能从 `Entity<SliderState>` 里读；而本项目的模型在 `AppShell` 里，
///    需要「值变了就写回模型」这一条路径。
/// 2. **手柄多了光环**。库里把手柄画成 `THUMB_RING_WIDTH = px(3.)` 的
///    `theme.ring` 光环，而真实规格的常态是**无边框的实心 `#44d62c` 圆**，
///    绿边只出现在 hover / active。
/// 3. **需要 `Entity<SliderState>`**。`SliderState` 要靠
///    `Window::use_keyed_state` 这类机制持有，而页面的渲染函数签名是
///    `fn render(app, cx)`，拿不到 `&mut Window`；要么改 24 个页面的签名
///    （涟漪很大），要么在渲染期往 `AppShell` 里塞 41 个实体（更糟）。
///
/// 因此按《Design Guides · Components and composition》
/// 「the application owns composition and product semantics」做成应用组件：
/// 颜色走令牌，几何走 [`geometry`] 里带出处注释的常量。
///
/// 拖动直接用 gpui 的鼠标事件：`MouseMoveEvent` 自带 `pressed_button`，
/// 因此不需要自己维护「正在拖动」的标志位；元素宽度由内部的 `canvas` 捕获。
pub fn slider_row(
    id: &'static str,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    display: String,
    enabled: bool,
    cx: &mut Context<AppShell>,
    set: impl Fn(&mut AppShell, f32, &mut Context<AppShell>) + 'static,
) -> AnyElement {
    use std::cell::Cell;
    use std::rc::Rc;

    let entity = cx.entity();
    let set: Rc<dyn Fn(&mut AppShell, f32, &mut Context<AppShell>)> = Rc::new(set);

    let track = rgba(geometry::SLIDER_TRACK_BG);
    let fill = rgb(geometry::SLIDER_FILL_BG);
    let thumb = rgb(geometry::SLIDER_THUMB_BG);
    let thumb_hover = cx.theme().border; // `.thumb:hover { background:#5d5d5d }`
    let label_color = cx.theme().muted_foreground; // `.title-more { color:#999 }`

    let span = (max - min).max(f32::EPSILON);
    let frac = ((value - min) / span).clamp(0., 1.);

    // 元素宽度在 prepaint 时才知道，用它在窗口坐标与 0–1 之间换算。
    let bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));

    // 把窗口横坐标换算成吸附到 step 的值。
    //
    // 鼠标回调给的是 `&mut App` 而不是 `Context<AppShell>`，因此这里接收 `&mut App`。
    let value_at = {
        let set = set.clone();
        let entity = entity.clone();
        let bounds = bounds.clone();
        move |x: f32, cx: &mut App| {
            let Some(b) = bounds.get() else {
                return;
            };
            let width = f32::from(b.size.width).max(f32::EPSILON);
            let ratio = ((x - f32::from(b.origin.x)) / width).clamp(0., 1.);
            let raw = min + ratio * span;
            let snapped = (((raw - min) / step).round() * step + min).clamp(min, max);
            let set = set.clone();
            entity.update(cx, |this, cx| set(this, snapped, cx));
        }
    };

    // 让 hover/active 的样式随状态变化，这里用 gpui 的 `.hover()` / `.active()`。
    let thumb_el = div()
        .absolute()
        .bottom(px(geometry::SLIDER_TRACK_BOTTOM + (geometry::SLIDER_TRACK_H
            - geometry::SLIDER_THUMB)
            / 2.))
        .left(relative(frac))
        .ml(px(-geometry::SLIDER_THUMB / 2.))
        .w(px(geometry::SLIDER_THUMB))
        .h(px(geometry::SLIDER_THUMB))
        .rounded(px(geometry::SLIDER_THUMB_RADIUS))
        .bg(thumb)
        // `.thumb:hover { background:#5d5d5d; border:2px solid #44d62c }`
        .hover(move |this| this.bg(thumb_hover).border_2().border_color(fill));

    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .gap_4()
        .child(div().text_sm().child(label.to_string()))
        .child(
            div()
                .relative()
                .flex_shrink_0()
                .w(px(geometry::SLIDER_W))
                .h(px(geometry::SLIDER_H))
                // `.slider-container { opacity:.3 }` / `.on { opacity:1 }`
                .when(enabled, |this| this.opacity(1.0))
                .when(!enabled, |this| {
                    this.opacity(geometry::SLIDER_DISABLED_OPACITY)
                })
                .child(
                    // `.track { bottom:25px; height:6px; width:100%;
                    //          background:#44d62c4d; border-radius:3px }`
                    div()
                        .absolute()
                        .bottom(px(geometry::SLIDER_TRACK_BOTTOM))
                        .left_0()
                        .right_0()
                        .h(px(geometry::SLIDER_TRACK_H))
                        .rounded(px(geometry::SLIDER_TRACK_RADIUS))
                        .bg(track),
                )
                .child(
                    // `.left { bottom:25px; height:6px; background:#44d62c }`
                    div()
                        .absolute()
                        .bottom(px(geometry::SLIDER_TRACK_BOTTOM))
                        .left_0()
                        .w(relative(frac))
                        .h(px(geometry::SLIDER_TRACK_H))
                        .rounded(px(geometry::SLIDER_TRACK_RADIUS))
                        .bg(fill),
                )
                .child(thumb_el)
                // `.foot { bottom:-2px; text-transform:uppercase }`
                .child(
                    div()
                        .absolute()
                        .bottom(px(geometry::SLIDER_FOOT_BOTTOM))
                        .text_sm()
                        .text_color(label_color)
                        .child(display),
                )
                .child(
                    canvas(
                        move |b, _, _| bounds.set(Some(b)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .id(id)
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, {
                    let value_at = value_at.clone();
                    move |event, _, cx| value_at(f32::from(event.position.x), cx)
                })
                .on_mouse_move({
                    let value_at = value_at.clone();
                    move |event, _, cx| {
                        // `MouseMoveEvent` 自带按下键，因此不需要拖动标志位。
                        if event.pressed_button == Some(MouseButton::Left) {
                            value_at(f32::from(event.position.x), cx);
                        }
                    }
                }),
        )
        .into_any_element()
}

/// 「◀ 值 ▶」步进行。
///
/// 说明：这是**暂留**实现。gpui-kit 的正确做法是用 [`Slider`]，但它要求每个
/// 数值都持有一个 `Entity<SliderState>`，需要在 `AppShell` 里为每个可调项
/// 保存状态，属于一次独立重构。在完成之前，
/// 这里保持可用的最小实现，不假装它是滑块。
pub fn stepper_row(
    label: &'static str,
    value: String,
    cx: &mut Context<AppShell>,
    dec: impl Fn(&mut AppShell, &mut Context<AppShell>) + 'static,
    inc: impl Fn(&mut AppShell, &mut Context<AppShell>) + 'static,
) -> AnyElement {
    let dec_id = format!("dec-{label}");
    let inc_id = format!("inc-{label}");

    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .gap_3()
        .child(div().text_sm().child(label))
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    btn(dec_id, "◀")
                        .on_click(cx.listener(move |this, _, _, cx| dec(this, cx))),
                )
                .child(div().min_w(px(150.)).text_center().text_sm().child(value))
                .child(
                    btn(inc_id, "▶")
                        .on_click(cx.listener(move |this, _, _, cx| inc(this, cx))),
                ),
        )
        .into_any_element()
}

/// DPI 档位可视化。
///
/// 用 gpui-kit 的 [`BarChart`] 绘制——这类图形属于 canvas 类渲染，
/// 应当交给组件库，而不是用一排 div 拼出近似效果。
///
/// `stages` 为 `(dpi, 是否当前档)`。
pub fn dpi_stage_chart(stages: &[(u32, bool)], cx: &App) -> AnyElement {
    if stages.is_empty() {
        return EmptyState::new("该设备没有 DPI 档位").into_any_element();
    }

    // 闭包要求 'static，所以先取出颜色，不要捕获 `cx`。
    let active_color: Hsla = cx.theme().primary;
    let idle_color: Hsla = cx.theme().border;

    let data: Vec<(usize, f64, bool)> = stages
        .iter()
        .enumerate()
        .map(|(index, (dpi, is_active))| (index, *dpi as f64, *is_active))
        .collect();

    BarChart::new(data)
        .id("dpi-stage-chart")
        .band(|(index, _, _)| format!("{}", index + 1))
        .value(|(_, dpi, _)| *dpi)
        .fill(move |(_, _, is_active), _, _, _| {
            if *is_active {
                active_color
            } else {
                idle_color
            }
        })
        .label(|(_, dpi, _)| format!("{dpi}"))
        .into_any_element()
}

/// 「写回设备」尚未接通时的说明。
///
/// 自定义页与灯光页都要显示它，因此放共享模块，避免两处措辞漂移。
///
/// 界面上必须**显式说明**这一点，而不是让用户以为改了就已经写进设备。
pub fn not_wired_hint() -> AnyElement {
    card()
        .child(card_title("写回设备"))
        .child(div().text_xs().child(
            "本实现复用雷云自己的后端引擎（simple_service.dll / razer_engine），\
             但写回通道尚未接通：当前改动只写入本地配置文件，不会下发到设备。",
        ))
        .child(div().text_xs().mt_1().child(
            "只读探测已可用（见 --probe）；写入路径见 docs/FEATURES.md §9。",
        ))
        .into_any_element()
}
