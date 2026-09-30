//! 应用外壳：窗口标题栏 + 顶栏（`.nav-tabs`）+ 内容区。
//!
//! # 结构与依据
//!
//! 外壳**没有左侧栏**，顶栏是**三个区**。逐条引自雷云主前端：
//!
//! ```jsx
//! <div className="main-container">                  // column, #222, min-width:600px
//!   <div className="nav-tabs">                      // min-height:48px, border-bottom:2px solid #000
//!     <div className="profile-wrapper">…</div>      // flex:1 0 25%
//!     <div className="navs-wrapper">…</div>         // flex:1 0 max-content, justify-content:center
//!     <div className="right">…</div>                // flex:1 1 25%
//!   </div>
//!   <div id="body-wrapper" className="body-wrapper">// flex:1, padding:10px 20px 20px
//!     …页面内容…
//!   </div>
//! </div>
//! ```
//!
//! 三区正好对应 gpui-kit [`TabBar`] 的 `prefix` / 标签项 / `suffix`
//! （库内有测试 `prefix_content_and_suffix_keep_their_order` 保证该顺序），
//! 因此顶栏用标准 [`TabBar`] + [`Tab::pill`]，而不是自己排一排 Button。
//!
//! 详见 [`docs/screens/00-app-shell.md`](../docs/screens/00-app-shell.md)。
//!
//! # 与原版的差异（已在文档中登记）
//!
//! 原版把「仪表盘 / 应用设置」与「设备页」做成**两个独立窗口**
//! （主前端 `openNewTab` 打开 `products/{id}/ui/index.html`）。本实现只有**一个窗口**，
//! 因此把设备选择放进顶栏左区，并用 `Tab::Home` / `Tab::Setting`
//! 这两个**应用级**标签页承载原版那两个页面。
//!
//! # 持久化
//!
//! 所有修改都会立即落到 [`crate::store`]，因为雷云的 IPC 层不提供持久化
//! （见 `store.rs` 的说明）。

// 完整图标目录（1830 个）在 `gpui_kit::assets::IconName`；
// `component::IconName` 只是原有子集。显式导入优先于 glob，因此这里的
// `IconName` 指完整目录——与 `nav.rs` 的写法一致。
use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::{
    DebounceMode,
    KEYBOARD_LAYOUTS, LiftOffDistance, LightingEffect, MACRO_ACTIONS, Macro, MacroStep,
    PollingRate,
};
use crate::model::{BUTTON_ACTIONS, Device};
use crate::nav::{DeviceKind, Tab};
use crate::features;
use crate::ui::widgets::geometry::TEXT_12;
use crate::store;
//  定义在 gpui 的 InteractiveElement trait 上——gpui_kit::*`r
// 转发的是 ::gpui::* 的根，不带这个 trait。


/// `.nav-tabs .nav:active { background-color:#3cbf27 }`。
///
/// 产品级显式值，全站 14 处均为同一角色（顶栏标签按下、顶栏图标盒悬停/按下、
/// 键区功能项选中）。它**不等于** `.thx-btn:hover{opacity:.8}` 混出的 `#3db22a`，
/// 也不等于 `.thx-btn:active{opacity:.6}` 混出的 `#368e28`，因此单列一个具名常量，
/// 而不是在调用点写十六进制、也不借用含义不同的主题令牌。

/// `div.nav-tabs { border-bottom:2px solid #000 }`。
///
/// 纯黑，**不是** `group_box` 的 `#111`。全站 `#000` 出现 132 次，其中
/// 「顶栏下边框」是独立角色，主题里没有对应档位，故在此具名。
const NAV_TABS_BORDER: u32 = 0x0000_00;

/// `div.nav-tabs { min-height:48px }`。
///
/// 注意：`gpui_kit::component::TitleBar` 内部写死了
/// `TITLE_BAR_HEIGHT = px(34.)`（`title_bar.rs:15`，用于 `title_bar.rs:335`
/// 的 `.h(TITLE_BAR_HEIGHT)`）。34px 是本项目**不能**采用的默认值，
/// 所以顶栏显式传这个 48px。
const APP_TABGROUP_HEIGHT: f32 = 42.0;
const WINDOW_CONTROL_WIDTH: f32 = 48.0;
const NAV_TABS_MIN_HEIGHT: f32 = 48.0;

/// 在字符串选项里按 `step` 环绕移动。
fn cycle_str(items: &[&'static str], current: &str, step: i32) -> String {
    let index = items.iter().position(|item| *item == current).unwrap_or(0) as i32;
    let len = items.len() as i32;
    items[((index + step).rem_euclid(len)) as usize].to_string()
}

/// 在预设色板里按 `step` 环绕移动。
// 导航在 `crate::nav`：**按设备**决定标签页，与雷云真实结构一致
// （每个产品一份独立模块，标签页写在该设备模块里）。见 `docs/RAZER-SYNAPSE-UI-SPEC.md` §2、§4。

/// 应用根视图。
pub struct AppShell {
    /// 当前顶栏标签页。应用级为 [`Tab::Home`] / [`Tab::Setting`]，
    /// 其余来自选中设备声明的标签页集合（见 [`crate::nav`]）。
    pub tab: Tab,
    /// 设备列表。首次启动来自本机实测快照，之后来自本地配置文件。
    pub devices: Vec<Device>,
    pub selected: usize,
    /// 最近一次保存结果，用于界面反馈。
    pub last_saved: Option<String>,
    /// 是否注入了合成演示设备（`--demo-keyboard`）。
    pub demo: bool,
    /// 全局亮度（`BRIGHTNESS_GLOBAL`）。作用于**所有设备**，所以放在应用级。
    pub global_brightness: crate::domain::GlobalBrightness,
    /// 配置文件切换方式（`PROFILE_SWITCHING`）。
    pub profile_switch_mode: crate::domain::ProfileSwitchMode,
    /// 已关联的游戏/程序（`LINKED_GAMES`）。
    pub linked_games: Vec<crate::domain::LinkedGame>,
    /// 首页各分段的**展开态**与**顺序**，对应雷云 Dashboard 的
    /// `.box-group`（`docs/screens/00-app-shell.md` §2.2）。
    ///
    /// 用同一个 `Vec` 同时表达顺序与展开态：`Vec` 的次序就是分段次序，
    /// 每项是 `(分段, 是否展开)`。这样「拖动重排」与「展开/折叠」共用一份状态，
    /// 不会出现两者不一致。
    pub home_sections: Vec<(HomeSection, bool)>,
    /// 当前展开的下拉（`.s3-dropdown`）。
    ///
    /// gpui-kit 的 `Popover` 自身能管理开合，但**选中后收起**需要在选项的
    /// 点击回调里改状态，而那里的上下文是 `&mut App`、拿不到
    /// `Context<PopoverState>`。因此这里做**受控**开合：状态放在应用侧，
    /// 选项点击时直接把它清空。
    pub open_select: Option<gpui::SharedString>,
    /// 是否有尚未写盘的改动。
    ///
    /// 驱动顶栏保存按钮上的红色角标（`.header-unsaved > .box > .badge`）：
    ///
    /// ```css
    /// .header-unsaved > .box > .badge { background-color:#c8323c; border-radius:9px;
    ///                                   font-size:10px; font-weight:700; height:18px;
    ///                                   line-height:19px; min-width:18px; padding:0 6px }
    /// ```
    ///
    /// 雷云是即时写盘的，所以它的角标表达的是「有改动正在保存」；
    /// 本实现把写盘做成显式动作（见 [`AppShell::save_now`]），
    /// 角标因此表达「有改动**待**保存」——语义更贴近用户看到的东西。
    pub dirty: bool,
}

/// 首页（Dashboard）的一个分段，对应雷云的 `.dashboard .box-group`。
///
/// 原版的分段是**原生的、可折叠可拖拽**的区块；这里列出本实现提供的分段。
/// 分段名沿用雷云的用词（`#devices` 来自其 CSS）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeSection {
    /// 设备（对应原版 `.dashboard .box-group #devices`）
    Devices,
    /// 已关联的游戏（`LINKED_GAMES`）
    LinkedGames,
    /// 引擎状态——**本实现自己的诊断区块**，原版没有
    Engines,
}

impl HomeSection {
    /// 全部分段，顺序即默认顺序。
    pub const ALL: [HomeSection; 3] = [
        HomeSection::Devices,
        HomeSection::LinkedGames,
        HomeSection::Engines,
    ];

    /// 分段标题。
    pub fn title(self) -> &'static str {
        match self {
            Self::Devices => "设备",
            Self::LinkedGames => "已关联的游戏",
            Self::Engines => "引擎状态（本实现的诊断区块）",
        }
    }

    /// 该分段默认是否展开。设备段默认展开——它是首页的主要任务。
    pub fn default_expanded(self) -> bool {
        match self {
            Self::Devices => true,
            Self::LinkedGames | Self::Engines => false,
        }
    }
}

impl Default for AppShell {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 视图
// ===========================================================================

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.app_tab_bar(window, cx))
            .child(self.top_bar(cx))
            .child(self.content(cx))
    }
}

impl AppShell {
    /// 当前应显示的标签页。
    ///
    /// 真实结构（`docs/screens/00-app-shell.md` §1）：**只有一行**标签，居中，
    /// 内容随选中设备变化——不是「应用标签 + 设备标签」两行。
    ///
    /// 因此这里是 `首页 ++ 该设备的标签 ++ 设置`。
    /// 每台设备的标签集合由 [`DeviceKind::tabs`] 给出，逐条来自实测的
    /// productId → 设备模块映射（182 鼠标 / 653 键盘 / 777 耳机）。
    pub fn tabs(&self) -> Vec<Tab> {
        let mut tabs = vec![Tab::Home];
        if let Some(device) = self.current() {
            tabs.extend_from_slice(DeviceKind::from_enum(device.category).tabs());
        }
        tabs.push(Tab::Setting);
        tabs
    }

    /// 顶栏。真实类名 `div.nav-tabs`。
    ///
    /// ```css
    /// div.nav-tabs { align-items:center; min-height:48px; position:relative; width:100%;
    ///                z-index:106; background-color:#222; border-bottom:2px solid #000;
    ///                color:#5d5d5d }
    /// ```
    ///
    /// **只有一个顶栏**：`.nav-tabs` 本身就是标题栏，雷云没有再单设一条。
    /// 三个区域的宽度比例逐字来自 CSS：
    ///
    /// ```css
    /// .nav-tabs .profile-wrapper { flex:1 0 25% }
    /// .nav-tabs .navs-wrapper    { flex:1 0 max-content; justify-content:center; font-size:12px }
    /// .nav-tabs .right           { flex:1 1 25% }
    /// ```
    fn top_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .flex_shrink_0()
            // `min-height:48px`
            .min_h(px(NAV_TABS_MIN_HEIGHT))
            .items_center()
            .relative()
            // `background-color:#222` —— 就是页面底色。
            .bg(cx.theme().background)
            // `border-bottom:2px solid #000` —— 纯黑，**不是** `group_box` 的 #111。
            .border_b_2()
            .border_color(rgb(NAV_TABS_BORDER))
            // `color:#5d5d5d` —— 顶栏的默认前景。
            .text_color(cx.theme().border)
            .child(self.profile_region(cx))
            .child(self.tab_strip(cx))
            .child(self.right_region(cx))
            .into_any_element()
    }

    fn app_tab_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let active_title = self
            .current()
            .map(|device| device.display_name())
            .unwrap_or_else(|| "雷云".to_string());

        h_flex()
            .w_full()
            .h(px(APP_TABGROUP_HEIGHT))
            .flex_shrink_0()
            .bg(theme.title_bar)
            .child(
                h_flex()
                    .id("app-tab-drag-region")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .items_center()
                    .gap_3()
                    .px(px(12.))
                    .child(Icon::new(IconName::Mouse).w(px(18.)).h(px(18.)))
                    .child(
                        div()
                            .text_size(px(TEXT_12))
                            .text_color(theme.foreground)
                            .child(active_title),
                    ),
            )
            .child(self.window_controls(window, &theme))
            .into_any_element()
    }

    fn window_controls(&self, window: &Window, theme: &Theme) -> AnyElement {
        let minimize = div()
            .id("window-minimize")
            .w(px(WINDOW_CONTROL_WIDTH))
            .h(px(APP_TABGROUP_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(WindowControlArea::Min)
            .hover(|style| style.bg(theme.background))
            .child(svg().data(include_bytes!("../assets/window-minimize.svg")).w(px(WINDOW_CONTROL_WIDTH)).h(px(32.)));

        let maximize_asset = if window.is_maximized() {
            "../assets/window-restore.svg"
        } else {
            "../assets/window-maximize.svg"
        };
        let maximize = div()
            .id("window-maximize")
            .w(px(WINDOW_CONTROL_WIDTH))
            .h(px(APP_TABGROUP_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(WindowControlArea::Max)
            .hover(|style| style.bg(theme.background))
            .child(svg().data(match maximize_asset {
                "../assets/window-restore.svg" => include_bytes!("../assets/window-restore.svg"),
                _ => include_bytes!("../assets/window-maximize.svg"),
            }).w(px(WINDOW_CONTROL_WIDTH)).h(px(32.)));

        let close = div()
            .id("window-close")
            .w(px(WINDOW_CONTROL_WIDTH))
            .h(px(APP_TABGROUP_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(WindowControlArea::Close)
            .hover(|style| style.bg(theme.background))
            .child(svg().data(include_bytes!("../assets/window-close.svg")).w(px(16.)).h(px(16.)));

        h_flex()
            .h_full()
            .flex_shrink_0()
            .child(minimize)
            .child(maximize)
            .child(close)
            .into_any_element()
    }

    /// 左区：配置文件选择。`flex:1 0 25%`。
    fn profile_region(&self, cx: &mut Context<Self>) -> AnyElement {
        let profile = self
            .current()
            .and_then(|device| device.active_profile_obj())
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| "无配置文件".to_string());

        h_flex()
            .flex_basis(gpui::relative(0.25))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px(px(10.))
            .child(div().text_xs().child("配置文件"))
            .child(
                div()
                    .id("profile-cycle")
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .cursor_pointer()
                    .child(profile)
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_profile(1, cx))),
            )
            .into_any_element()
    }

    /// 中区：标签条。`flex:1 0 max-content; justify-content:center; font-size:12px`。
    ///
    /// `.nav` 的完整规格：
    ///
    /// ```css
    /// .nav-tabs .nav { background-repeat:no-repeat; border-radius:14px; color:#999;
    ///                  line-height:14px; margin-right:20px; padding:7px 10px;
    ///                  text-align:center; text-transform:uppercase;
    ///                  transition:background-color .3s,color .1s; white-space:nowrap }
    /// .nav-tabs .nav:hover        { background-color:#2d2d2d; color:#ccc }
    /// .nav-tabs .nav:active       { background-color:#3cbf27; color:#111 }
    /// .nav.active,
    /// .nav-tabs .nav.active:hover { background-color:#44d62c; color:#111 }
    /// .nav.disabled               { opacity:.3; pointer-events:none }
    /// ```
    ///
    /// **`.nav` 自己没有 `font-size`**——它继承 `.navs-wrapper` 的 `12px`。
    /// 这点很容易做错（gpui-kit 的小号控件默认 14px），所以在容器上显式写
    /// `text_size(TEXT_12)`。
    fn tab_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.tab;
        let theme = cx.theme().clone();

        h_flex()
            .flex_basis(gpui::relative(1.))
            .flex_shrink_0()
            .justify_center()
            .items_center()
            .text_size(px(TEXT_12))
            .children(self.tabs().into_iter().map(|tab| {
                let is_active = tab == active;
                div()
                    .id(("nav", tab as usize))
                    .flex()
                    .items_center()
                    .justify_center()
                    // `border-radius:14px`
                    .rounded(px(14.))
                    // `line-height:14px; padding:7px 10px; margin-right:20px`
                    .px(px(10.))
                    .py(px(7.))
                    .mr(px(20.))
                    // `white-space:nowrap` + `text-transform:uppercase`
                    .whitespace_nowrap()
                    .child(tab.zh().to_uppercase())
                    .when(is_active, |this| {
                        // `.nav.active { background-color:#44d62c; color:#111 }`
                        this.bg(theme.primary).text_color(theme.primary_foreground)
                    })
                    .when(!is_active, |this| {
                        // `.nav { color:#999 }`，悬停 `#2d2d2d` / `#ccc`
                        this.text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(|style| {
                                style
                                    .bg(theme.secondary_hover)
                                    .text_color(theme.foreground)
                            })
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    /// 右区：电量与帮助。`flex:1 1 25%`。
    fn right_region(&self, cx: &mut Context<Self>) -> AnyElement {
        let battery = self
            .current()
            .and_then(|device| device.power_status.as_ref())
            .map(|status| format!("{} %", status.level));
        let theme = cx.theme().clone();
        let help_hover = theme.secondary_hover;

        h_flex()
            .flex_basis(gpui::relative(0.25))
            .flex_grow(1.)
            .flex_shrink(1.)
            .justify_end()
            .items_center()
            .gap_2()
            .px(px(10.))
            .when_some(battery, |this, text| {
                this.child(div().text_xs().child(text))
            })
            .when(self.dirty, |this| {
                this.child(div().text_xs().text_color(theme.primary).child("*"))
            })
            .child(
                // `.nav-tabs .help { align-items:center; display:flex; height:24px;
                //                    justify-content:center; margin-right:10px;
                //                    width:24px; border-radius:5px }`
                div()
                    .id("topbar-help")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(24.))
                    .h(px(24.))
                    .mr(px(10.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(help_hover))
                    .child(Icon::new(IconName::Info).w(px(16.)).h(px(16.))),
            )
            .into_any_element()
    }

    /// 内容区：按当前标签路由到对应页面。
    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let page: AnyElement = match self.tab {
            Tab::Home => features::dashboard::render(self, cx),
            Tab::Setting => features::setting::render(self, cx),
            Tab::Customize => features::customize::render(self, cx),
            Tab::Performance => features::performance::render(self, cx),
            Tab::Pairing => features::pairing::render(self, cx),
            Tab::Calibration => features::calibration::render(self, cx),
            Tab::Power => features::power::render(self, cx),
            Tab::Scrolling => features::scrolling::render(self, cx),
            Tab::Lighting => features::lighting::render(self, cx),
            Tab::Sound => features::sound::render(self, cx),
            Tab::Mic => features::mic::render(self, cx),
            Tab::Mixer => features::mixer::render(self, cx),
            Tab::Audio => features::audio::render(self, cx),
            Tab::Enhancement => features::enhancement::render(self, cx),
            Tab::Eq => features::eq::render(self, cx),
            Tab::Haptics => features::haptics::render(self, cx),
            Tab::Display => features::display::render(self, cx),
            Tab::Oled => features::oled::render(self, cx),
            Tab::Keyboard => features::keyboard::render(self, cx),
            Tab::Macros => features::macros::render(self, cx),
            Tab::Battery => features::power::render(self, cx),
            Tab::Color | Tab::Effects => features::lighting::render(self, cx),
            Tab::Gaming => {
                if self.current().is_some_and(|device| device.is_keyboard()) {
                    features::keyboard::render(self, cx)
                } else {
                    features::performance::render(self, cx)
                }
            }
            Tab::KeyBinds => features::customize::render(self, cx),
            Tab::Demo => features::dashboard::render(self, cx),
        };

        // `.body-wrapper { flex:1 1; min-width:600px; padding:10px 20px 20px }`
        div()
            .flex_grow(1.)
            .flex_shrink(1.)
            .min_w(px(crate::ui::widgets::geometry::SHELL_MIN_WIDTH))
            .px(px(20.))
            .pt(px(10.))
            .pb(px(20.))
            
            .child(page)
            .into_any_element()
    }
}

impl AppShell {
    /// 构造应用状态。
    ///
    /// **自己读命令行参数**（`main.rs` 构造它时不传参）：
    ///
    /// | 参数 | 作用 |
    /// |---|---|
    /// | `--demo-keyboard` | 在实测设备之外再注入一台合成键盘 |
    /// | `--tab <名>` | 启动时选中某个标签页（接受变体名或 `TAB_*`） |
    ///
    /// 设备来源：优先读本地配置（`store::load`），没有才回落到**实测快照**
    /// （`model::measured_devices`）。这样界面上改过的设置不会在重启后丢失。
    pub fn new() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let demo = args.iter().any(|arg| arg == "--demo-keyboard");

        let mut devices = match crate::store::load() {
            Some(saved) => saved,
            None => crate::model::measured_devices(),
        };
        if demo {
            devices.push(crate::demo::demo_keyboard());
        }

        // `--tab` 既接受 `customize` 也接受 `TAB_CUSTOMIZE`。
        let tab = args
            .iter()
            .position(|arg| arg == "--tab")
            .and_then(|index| args.get(index + 1))
            .and_then(|value| Tab::from_arg(value))
            .unwrap_or(Tab::Home);

        Self {
            tab,
            devices,
            selected: 0,
            last_saved: None,
            demo,
            global_brightness: crate::domain::GlobalBrightness::default(),
            profile_switch_mode: crate::domain::ProfileSwitchMode::Automatic,
            linked_games: Vec::new(),
            home_sections: default_home_sections(),
            open_select: None,
            dirty: false,
        }
    }

    // ==================== 基础 ====================

    /// 当前选中的设备。
    pub fn current(&self) -> Option<&Device> {
        self.devices.get(self.selected)
    }

    /// 当前选中的设备（可变）。
    pub fn current_mut(&mut self) -> Option<&mut Device> {
        self.devices.get_mut(self.selected)
    }

    /// 改当前设备的 [`DeviceFeatures`]，并通知界面。
    ///
    /// 所有 `toggle_*` / `adjust_*` / `set_*` 都走这里，所以「改了就刷新」
    /// 这条规则只有一处实现。没有选中设备时**静默返回**——界面在无设备时
    /// 本就不渲染这些控件。
    pub fn edit_features(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut crate::domain::DeviceFeatures),
    ) {
        let Some(device) = self.devices.get_mut(self.selected) else {
            return;
        };
        edit(&mut device.features);
        self.dirty = true;
        cx.notify();
    }

    /// 改当前设备本身（不只是 features）。
    pub fn edit_device(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Device),
    ) {
        let Some(device) = self.devices.get_mut(self.selected) else {
            return;
        };
        edit(device);
        cx.notify();
    }

    /// 翻转某个布尔字段。所有 `toggle_*` 共用。
    fn flip(
        &mut self,
        cx: &mut Context<Self>,
        pick: impl FnOnce(&mut crate::domain::DeviceFeatures) -> Option<&mut bool>,
    ) {
        self.edit_features(cx, |features| {
            if let Some(value) = pick(features) {
                *value = !*value;
            }
        });
    }

    /// 按 `delta` 调整某个 `u8` 百分比字段，夹到 `0..=100`。
    fn bump_u8(
        &mut self,
        cx: &mut Context<Self>,
        delta: i32,
        step: i32,
        pick: impl FnOnce(&mut crate::domain::DeviceFeatures) -> Option<&mut u8>,
    ) {
        self.edit_features(cx, |features| {
            if let Some(value) = pick(features) {
                *value = (*value as i32 + delta * step).clamp(0, 100) as u8;
            }
        });
    }

    /// 把某个 `u8` 百分比字段设成 `value`（滑块用；滑块给的是 `f32`）。
    pub fn set_percent_u8(
        &mut self,
        cx: &mut Context<Self>,
        value: f32,
        pick: impl FnOnce(&mut crate::domain::DeviceFeatures) -> Option<&mut u8>,
    ) {
        self.edit_features(cx, |features| {
            if let Some(slot) = pick(features) {
                *slot = value.clamp(0., 100.) as u8;
            }
        });
    }

    // ==================== 配置文件 ====================

    /// 直接设定配置文件切换方式（自动 / 手动）。
    ///
    /// 二选一的枚举 → 真实界面是 `.s3-dropdown` 直选。
    pub fn set_profile_switch_mode(
        &mut self,
        mode: crate::domain::ProfileSwitchMode,
        cx: &mut Context<Self>,
    ) {
        self.profile_switch_mode = mode;
        cx.notify();
    }

    /// 选中第 `index` 个配置文件。
    pub fn select_profile(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit_device(cx, |device| {
            if let Some(profile) = device.profiles.get(index) {
                device.active_profile = profile.id.clone();
            }
        });
        self.last_saved = Some("已切换配置文件".to_string());
        cx.notify();
    }

    /// 循环切换配置文件（`cycle_profile(-1)` / `cycle_profile(1)`）。
    pub fn cycle_profile(&mut self, step: i32, cx: &mut Context<Self>) {
        let Some(device) = self.current() else {
            return;
        };
        if device.profiles.is_empty() {
            return;
        }
        let current = device
            .profiles
            .iter()
            .position(|p| p.id == device.active_profile)
            .unwrap_or(0) as i32;
        let len = device.profiles.len() as i32;
        let next = (current + step).rem_euclid(len) as usize;
        self.select_profile(next, cx);
    }

    // ==================== 设备列表 ====================

    /// 选中第 `index` 台设备。
    pub fn select_device(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.devices.len() {
            self.selected = index;
            // 换设备后回到该设备的第一个标签页，避免停留在上一台设备
            // 才有、这台没有的页面上。
            let kind = DeviceKind::from_enum(self.devices[index].category);
            self.tab = kind.tabs().first().copied().unwrap_or(Tab::Home);
            cx.notify();
        }
    }

    pub fn select_device_by_serial(&mut self, serial_number: &str, cx: &mut Context<Self>) {
        if let Some(index) = self
            .devices
            .iter()
            .position(|device| device.serial_number == serial_number)
        {
            self.select_device(index, cx);
        }
    }

    /// 删除第 `index` 台设备（首页的「移除」）。
    pub fn remove_device(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.devices.len() {
            return;
        }
        self.devices.remove(index);
        if self.selected >= self.devices.len() {
            self.selected = self.devices.len().saturating_sub(1);
        }
        self.last_saved = Some("已移除设备".to_string());
        cx.notify();
    }

    // ==================== 首页分段 ====================

    /// 折叠/展开首页的某个分段。
    pub fn toggle_home_section(&mut self, section: HomeSection, cx: &mut Context<Self>) {
        if let Some(entry) = self.home_sections.iter_mut().find(|(s, _)| *s == section) {
            entry.1 = !entry.1;
            cx.notify();
        }
    }

    /// 上移/下移某个首页分段（`delta` 为 -1 / 1）。
    ///
    /// 用「上移」「下移」按钮而不是拖拽：拖拽在纯鼠标之外没有等价操作，
    /// 而《Design Guides · Accessibility》要求每个动作都有键盘可达的路径。
    pub fn move_home_section(&mut self, section: HomeSection, delta: i32, cx: &mut Context<Self>) {
        let Some(index) = self.home_sections.iter().position(|(s, _)| *s == section) else {
            return;
        };
        let target = index as i32 + delta;
        if target < 0 || target as usize >= self.home_sections.len() {
            return;
        }
        self.home_sections.swap(index, target as usize);
        cx.notify();
    }

    // ==================== 已关联的游戏 ====================

    /// 添加一条已关联的游戏/程序（`ADD_GAME_TITLE`）。
    pub fn add_linked_game(&mut self, cx: &mut Context<Self>) {
        let n = self.linked_games.len() + 1;
        self.linked_games
            .push(crate::domain::LinkedGame::new(&format!(
                "游戏或程序 {n}"
            )));
        self.last_saved = Some(format!("已添加「游戏或程序 {n}」"));
        cx.notify();
    }

    /// 删除一条已关联的游戏/程序。
    pub fn remove_linked_game(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.linked_games.len() {
            let removed = self.linked_games.remove(index);
            self.last_saved = Some(format!("已从游戏库删除「{}」", removed.game));
            cx.notify();
        }
    }

    /// 给某条已关联的游戏指定配置文件。
    pub fn set_linked_game_profile(
        &mut self,
        index: usize,
        profile_guid: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(game) = self.linked_games.get_mut(index) {
            game.profile_guid = profile_guid;
            cx.notify();
        }
    }

    /// 翻转某条游戏的 Chroma 联动（`LINKED_GAME_CHROMA`）。
    pub fn toggle_linked_game_chroma(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(game) = self.linked_games.get_mut(index) {
            game.chroma = !game.chroma;
            cx.notify();
        }
    }

    // ==================== 下拉开合 ====================

    /// 设置当前展开的下拉（`.s3-dropdown` 的受控开合）。
    pub fn set_open_select(
        &mut self,
        id: Option<gpui::SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.open_select = id;
        cx.notify();
    }

    // ==================== 持久化 ====================

    /// 把当前设备列表写入本地配置。
    ///
    /// 雷云的改动是**即时写盘**的；这里做成显式动作，
    /// 因为拖动滑块时每次 `on_mouse_move` 都写文件是浪费。
    pub fn save_now(&mut self, cx: &mut Context<Self>) {
        self.last_saved = Some(match store::save(&self.devices) {
            Ok(path) => format!("已保存到 {}", path.display()),
            Err(err) => format!("保存失败：{err}"),
        });
        cx.notify();
    }

    // ==================== 性能 ====================

    /// 直接设定轮询率（`POLLING_RATE`）。
    pub fn set_polling_rate(&mut self, rate: PollingRate, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| f.performance.polling_rate = rate);
    }

    /// 直接设定抬升距离（`LIFT_OFF_DISTANCE`）。
    pub fn set_lift_off(&mut self, value: LiftOffDistance, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| f.performance.lift_off = Some(value));
    }

    /// 直接设定传感器回弹模式（`DEBOUNCE_MODE`）。
    pub fn set_debounce_mode(&mut self, mode: DebounceMode, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| f.performance.debounce_mode = Some(mode));
    }

    pub fn toggle_acceleration(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.performance.acceleration.as_mut());
    }

    // ==================== 灯光 ====================

    /// 直接设定某个分区的灯光效果（`LIGHTING_EFFECT`）。
    pub fn set_zone_effect(
        &mut self,
        index: usize,
        effect: LightingEffect,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(zone) = f.lighting.get_mut(index) {
                zone.effect = effect;
            }
        });
    }

    /// 调整某个分区的亮度（`0–100`，滑块给 `f32`）。
    pub fn set_zone_brightness(
        &mut self,
        index: usize,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(zone) = f.lighting.get_mut(index) {
                zone.brightness = value.clamp(0., 100.) as u8;
            }
        });
    }

    /// 调整某个分区的速度（`0–100`）。
    pub fn set_zone_speed(&mut self, index: usize, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(zone) = f.lighting.get_mut(index) {
                zone.speed = value.clamp(0., 100.) as u8;
            }
        });
    }

    pub fn toggle_global_brightness(&mut self, cx: &mut Context<Self>) {
        self.global_brightness.enabled = !self.global_brightness.enabled;
        cx.notify();
    }

    /// 直接设定全局亮度（`BRIGHTNESS_GLOBAL`）。
    pub fn set_global_brightness(&mut self, value: f32, cx: &mut Context<Self>) {
        self.global_brightness.level = value.clamp(0., 100.) as u8;
        cx.notify();
    }

    pub fn adjust_global_brightness(&mut self, delta: i32, cx: &mut Context<Self>) {
        let level = self.global_brightness.level as i32 + delta * 5;
        self.global_brightness.level = level.clamp(0, 100) as u8;
        cx.notify();
    }

    pub fn toggle_lighting_on_battery(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.power.as_mut().map(|p| &mut p.lighting_on_battery)
        });
    }

    /// 调整「无活动后调暗」（分钟；0 = 已关闭）。
    pub fn adjust_dim_on_battery(&mut self, delta: i32, cx: &mut Context<Self>) {
        use crate::domain::DIM_ON_BATTERY_STEPS;
        self.edit_features(cx, |f| {
            if let Some(kb) = f.keyboard.as_mut() {
                let current = DIM_ON_BATTERY_STEPS
                    .iter()
                    .position(|v| *v == kb.dim_on_battery_after_min)
                    .unwrap_or(0) as i32;
                let next = (current + delta)
                    .rem_euclid(DIM_ON_BATTERY_STEPS.len() as i32)
                    as usize;
                kb.dim_on_battery_after_min = DIM_ON_BATTERY_STEPS[next];
            }
        });
    }

    // ==================== 键盘 ====================

    pub fn toggle_gaming_mode(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.keyboard.as_mut().map(|k| &mut k.gaming_mode));
    }

    pub fn toggle_lock_alt_tab(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.keyboard.as_mut().map(|k| &mut k.lock_alt_tab));
    }

    pub fn toggle_lock_alt_f4(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.keyboard.as_mut().map(|k| &mut k.lock_alt_f4));
    }

    pub fn toggle_n_key_rollover(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.keyboard.as_mut().map(|k| &mut k.n_key_rollover));
    }

    pub fn toggle_actuation(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.keyboard
                .as_mut()
                .and_then(|k| k.actuation.as_mut())
                .map(|a| &mut a.enabled)
        });
    }

    pub fn toggle_actuation_feedback(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.keyboard
                .as_mut()
                .and_then(|k| k.actuation.as_mut())
                .map(|a| &mut a.feedback)
        });
    }

    pub fn toggle_rapid_trigger(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.keyboard
                .as_mut()
                .and_then(|k| k.actuation.as_mut())
                .map(|a| &mut a.rapid_trigger)
        });
    }

    /// 循环切换键盘布局（`KEYBOARD_LAYOUT`）。
    ///
    /// `KEYBOARD_LAYOUTS` 是 `(语言代码, 显示名)` 的键值对，
    /// 而 `KeyboardSettings::layout` 存的是**语言代码**，所以只取第一项来循环。
    pub fn cycle_keyboard_layout(&mut self, step: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(kb) = f.keyboard.as_mut() {
                let codes: Vec<&str> = KEYBOARD_LAYOUTS.iter().map(|(code, _)| *code).collect();
                let current = codes.iter().position(|c| *c == kb.layout).unwrap_or(0) as i32;
                let next = (current + step).rem_euclid(codes.len() as i32) as usize;
                kb.layout = codes[next].to_string();
            }
        });
    }

    /// 调整背光自动关闭时间（秒；0 = 常亮）。
    pub fn adjust_backlight_timeout(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(kb) = f.keyboard.as_mut() {
                let current = kb.backlight_timeout_sec as i32 / 15;
                let next = (current + delta).clamp(0, 20);
                kb.backlight_timeout_sec = (next * 15) as u16;
            }
        });
    }

    /// 直接设定背光自动关闭时间（秒）。
    pub fn set_backlight_timeout(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(kb) = f.keyboard.as_mut() {
                kb.backlight_timeout_sec = value.clamp(0., 300.) as u16;
            }
        });
    }

    pub fn toggle_snap_tap(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.keyboard
                .as_mut()
                .and_then(|k| k.snap_tap.as_mut())
                .map(|s| &mut s.enabled)
        });
    }

    pub fn add_snap_tap_pair(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(snap) = f.keyboard.as_mut().and_then(|k| k.snap_tap.as_mut()) {
                if snap.pairs.len() < crate::domain::SNAP_TAP_MAX_PAIRS {
                    snap.pairs.push(crate::domain::SnapTapPair::new("A", "D"));
                }
            }
        });
    }

    pub fn remove_snap_tap_pair(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(snap) = f.keyboard.as_mut().and_then(|k| k.snap_tap.as_mut()) {
                snap.pairs.pop();
            }
        });
    }

    /// 直接设定某一组的录入模式（五选一）。
    pub fn set_snap_tap_mode(
        &mut self,
        index: usize,
        mode: crate::domain::SnapTapMode,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(snap) = f.keyboard.as_mut().and_then(|k| k.snap_tap.as_mut()) {
                if let Some(pair) = snap.pairs.get_mut(index) {
                    pair.mode = mode;
                }
            }
        });
    }

    pub fn toggle_dynamic_key_stroke(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.keyboard
                .as_mut()
                .and_then(|k| k.dynamic_key_stroke.as_mut())
                .map(|d| &mut d.enabled)
        });
    }

    /// 调整按键行程灵敏度（毫米，`0.1–4.0`）。
    ///
    /// `press` 为真时改「按下」侧，否则改「释放」侧。
    pub fn adjust_key_stroke_sensitivity(
        &mut self,
        press: bool,
        delta: f32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(dks) = f.keyboard.as_mut().and_then(|k| k.dynamic_key_stroke.as_mut()) {
                let slot = if press {
                    &mut dks.press_start_sensitivity
                } else {
                    &mut dks.press_end_sensitivity
                };
                *slot = (*slot + delta).clamp(0.1, 4.0);
            }
        });
    }

    /// 直接设定按键行程灵敏度（毫米）。
    pub fn set_key_stroke_sensitivity(
        &mut self,
        press: bool,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(dks) = f.keyboard.as_mut().and_then(|k| k.dynamic_key_stroke.as_mut()) {
                let slot = if press {
                    &mut dks.press_start_sensitivity
                } else {
                    &mut dks.press_end_sensitivity
                };
                *slot = value.clamp(0.1, 4.0);
            }
        });
    }

    /// 调整主触发距离（毫米）。
    pub fn adjust_primary_actuation(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(a) = f.keyboard.as_mut().and_then(|k| k.actuation.as_mut()) {
                a.primary_mm = (a.primary_mm + delta).clamp(0.1, 4.0);
            }
        });
    }

    /// 调整第二触发距离（毫米）。
    pub fn adjust_secondary_actuation(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(a) = f.keyboard.as_mut().and_then(|k| k.actuation.as_mut()) {
                a.secondary_mm = (a.secondary_mm + delta).clamp(0.1, 4.0);
            }
        });
    }

    /// 直接设定主触发距离（毫米）。
    pub fn set_primary_actuation(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(a) = f.keyboard.as_mut().and_then(|k| k.actuation.as_mut()) {
                a.primary_mm = value.clamp(0.1, 4.0);
            }
        });
    }

    /// 直接设定第二触发距离（毫米）。
    pub fn set_secondary_actuation(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(a) = f.keyboard.as_mut().and_then(|k| k.actuation.as_mut()) {
                a.secondary_mm = value.clamp(0.1, 4.0);
            }
        });
    }

    pub fn toggle_boss_key(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.boss_key.as_mut().map(|b| &mut b.enabled));
    }

    pub fn toggle_key_shifter(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.key_shifter.as_mut().map(|k| &mut k.enabled));
    }

    pub fn adjust_key_shifter_pitch(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(k) = f.key_shifter.as_mut() {
                k.pitch = (k.pitch as i32 + delta).clamp(-12, 12) as i8;
            }
        });
    }

    pub fn set_key_shifter_pitch(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(k) = f.key_shifter.as_mut() {
                k.pitch = value.clamp(-12., 12.) as i8;
            }
        });
    }

    /// 调整变声速度百分比。`KeyShifter::speed` 是 `u16`，所以不共用 `u8` 的辅助。
    pub fn adjust_key_shifter_speed(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(k) = f.key_shifter.as_mut() {
                k.speed = (k.speed as i32 + delta * 5).clamp(0, 100) as u16;
            }
        });
    }

    pub fn set_key_shifter_speed(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(k) = f.key_shifter.as_mut() {
                k.speed = value.clamp(0., 100.) as u16;
            }
        });
    }

    /// 直接设定某个 Hypershift 绑定的动作。
    ///
    /// `BUTTON_ACTIONS` 是固定集合 → 真实界面用 `.s3-dropdown` 直选。
    pub fn set_hypershift_action(&mut self, index: usize, picked: usize, cx: &mut Context<Self>) {
        let Some(action) = BUTTON_ACTIONS.get(picked) else {
            return;
        };
        let action = action.to_string();
        self.edit_features(cx, |f| {
            if let Some(binding) = f.hypershift_bindings.get_mut(index) {
                binding.button_key = action;
            }
        });
    }

    /// 直接设定某个物理输入点的动作（自定义页的主表）。
    pub fn set_binding_action(&mut self, index: usize, picked: usize, cx: &mut Context<Self>) {
        let Some(action) = BUTTON_ACTIONS.get(picked) else {
            return;
        };
        let action = action.to_string();
        self.edit_device(cx, |device| {
            if let Some(binding) = device.dkm_keys.get_mut(index) {
                binding.button_key = action;
            }
        });
    }

    pub fn toggle_hypershift(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| Some(&mut f.hypershift_enabled));
    }

    // ==================== 电源 ====================

    /// 调整闲置休眠时间（分钟；0 = 从不）。
    pub fn adjust_sleep_after(&mut self, delta: i32, cx: &mut Context<Self>) {
        use crate::domain::SLEEP_AFTER_STEPS;
        self.edit_features(cx, |f| {
            if let Some(p) = f.power.as_mut() {
                let current =
                    SLEEP_AFTER_STEPS.iter().position(|v| *v == p.sleep_after_min).unwrap_or(0)
                        as i32;
                let next =
                    (current + delta).rem_euclid(SLEEP_AFTER_STEPS.len() as i32) as usize;
                p.sleep_after_min = SLEEP_AFTER_STEPS[next];
            }
        });
    }

    /// 调整闲置降低亮度时间（分钟；0 = 从不）。
    pub fn adjust_dim_after(&mut self, delta: i32, cx: &mut Context<Self>) {
        use crate::domain::DIM_AFTER_STEPS;
        self.edit_features(cx, |f| {
            if let Some(p) = f.power.as_mut() {
                let current =
                    DIM_AFTER_STEPS.iter().position(|v| *v == p.dim_after_min).unwrap_or(0) as i32;
                let next = (current + delta).rem_euclid(DIM_AFTER_STEPS.len() as i32) as usize;
                p.dim_after_min = DIM_AFTER_STEPS[next];
            }
        });
    }

    pub fn toggle_low_power_mode(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.power.as_mut().map(|p| &mut p.low_power_mode));
    }

    pub fn toggle_battery_health_optimizer(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.power.as_mut().map(|p| &mut p.battery_health_optimizer)
        });
    }

    pub fn adjust_battery_health_threshold(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| {
            f.power.as_mut().map(|p| &mut p.battery_health_threshold)
        });
    }

    pub fn set_battery_health_threshold(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| {
            f.power.as_mut().map(|p| &mut p.battery_health_threshold)
        });
    }

    pub fn adjust_brightness_when_active(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| {
            f.power.as_mut().map(|p| &mut p.brightness_when_active)
        });
    }

    pub fn set_brightness_when_active(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| {
            f.power.as_mut().map(|p| &mut p.brightness_when_active)
        });
    }

    // ==================== 滚动 ====================

    /// 直接设定滚动模式（`SCROLL_MODE` / `FREE_SPIN`）。
    pub fn set_scrolling_mode(
        &mut self,
        mode: crate::domain::ScrollingMode,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.scrolling.as_mut() {
                s.mode = mode;
            }
        });
    }

    /// 调整滚轮每转级数（`SCROLL_STEPS`）。
    pub fn adjust_scroll_steps(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.scrolling.as_mut() {
                s.steps = (s.steps as i32 + delta).clamp(1, 60) as u8;
            }
        });
    }

    pub fn set_scroll_steps(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.scrolling.as_mut() {
                s.steps = value.clamp(1., 60.) as u8;
            }
        });
    }

    /// 调整滚动阻力（`SCROLL_TENSION`，0–100）。
    pub fn adjust_scroll_tension(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.scrolling.as_mut().map(|s| &mut s.tension));
    }

    pub fn set_scroll_tension(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.scrolling.as_mut().map(|s| &mut s.tension));
    }

    /// 调整某个触觉等级的强度（`SCROLL_STAGE_HAPTICS`）。
    pub fn adjust_scroll_stage_haptics(&mut self, index: usize, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.scrolling.as_mut() {
                if let Some(stage) = s.stages.get_mut(index) {
                    stage.haptics = (stage.haptics as i32 + delta * 5).clamp(0, 100) as u8;
                }
            }
        });
    }

    pub fn set_scroll_stage_haptics(&mut self, index: usize, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.scrolling.as_mut() {
                if let Some(stage) = s.stages.get_mut(index) {
                    stage.haptics = value.clamp(0., 100.) as u8;
                }
            }
        });
    }

    /// 启用/禁用某个触觉等级。
    ///
    /// 雷云**最多允许禁用 2 个等级**（`SCROLL_DISABLE_MODES_DESC`），
    /// 所以这里带这个上限检查——不是实现细节，是产品规则。
    pub fn toggle_scroll_stage_disabled(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            let Some(s) = f.scrolling.as_mut() else {
                return;
            };
            let disabled = s.stages.iter().filter(|stage| stage.disabled).count();
            let Some(stage) = s.stages.get_mut(index) else {
                return;
            };
            if stage.disabled {
                stage.disabled = false;
            } else if disabled < crate::domain::SCROLL_MAX_DISABLED_STAGES {
                stage.disabled = true;
            }
        });
    }

    pub fn toggle_scroll_acceleration(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.scrolling.as_mut().map(|s| &mut s.acceleration));
    }

    pub fn toggle_high_resolution_scrolling(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.scrolling.as_mut().map(|s| &mut s.high_resolution)
        });
    }

    pub fn toggle_horizontal_scrolling(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.scrolling.as_mut().map(|s| &mut s.horizontal));
    }

    pub fn toggle_scroll_haptics(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.scrolling.as_mut().map(|s| &mut s.haptics_enabled)
        });
    }

    // ==================== 校准 ====================

    /// 选中第 `index` 个表面。
    pub fn select_surface(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(c) = f.calibration.as_mut() {
                if index < c.surfaces.len() {
                    c.selected = index;
                }
            }
        });
    }

    /// 新增一个自定义表面（`ADD_SURFACE`）。
    pub fn add_surface(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(c) = f.calibration.as_mut() {
                let n = c.surfaces.len() + 1;
                c.surfaces
                    .push(crate::domain::SurfaceProfile::new(&format!("表面 {n}"), false));
            }
        });
    }

    /// 删除当前选中的自定义表面。内置表面**不可删**。
    pub fn remove_surface(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(c) = f.calibration.as_mut() {
                let index = c.selected;
                let removable = c
                    .surfaces
                    .get(index)
                    .map(|s| !s.builtin)
                    .unwrap_or(false);
                if removable {
                    c.surfaces.remove(index);
                    c.selected = c.selected.min(c.surfaces.len().saturating_sub(1));
                }
            }
        });
    }

    /// 开始校准当前选中的表面。
    pub fn start_calibration(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(c) = f.calibration.as_mut() {
                c.state = crate::domain::CalibrationState::Running;
            }
        });
    }

    /// 结束校准。`success` 为假表示取消，不写入结果。
    pub fn finish_calibration(&mut self, success: bool, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(c) = f.calibration.as_mut() {
                c.state = if success {
                    crate::domain::CalibrationState::Completed
                } else {
                    crate::domain::CalibrationState::Idle
                };
                if success {
                    let index = c.selected;
                    if let Some(surface) = c.surfaces.get_mut(index) {
                        surface.calibrated = true;
                    }
                }
            }
        });
    }

    // ==================== 配对 ====================

    pub fn toggle_pairing(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(p) = f.pairing.as_mut() {
                p.pairing = !p.pairing;
            }
        });
    }

    /// 完成一次配对（把当前接收器与一台设备配起来）。
    pub fn complete_pairing(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(p) = f.pairing.as_mut() {
                let n = p.paired_devices.len() + 1;
                p.paired_devices.push(format!("已配对设备 {n}"));
                p.pairing = false;
            }
        });
        self.last_saved = Some("配对完成".to_string());
        cx.notify();
    }

    /// 解除全部配对（`UNPAIR_ALL`）。
    pub fn unpair_all(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(p) = f.pairing.as_mut() {
                p.paired_devices.clear();
                p.pairing = false;
            }
        });
        self.last_saved = Some("已解除全部配对".to_string());
        cx.notify();
    }

    /// 循环切换接收器型号（`DONGLE_KIND`）。
    ///
    /// `Pairing::dongle` 是枚举而不是字符串，所以用 match 循环，不套 `cycle_str`。
    pub fn cycle_dongle_kind(&mut self, cx: &mut Context<Self>) {
        use crate::domain::DongleKind;
        self.edit_features(cx, |f| {
            if let Some(p) = f.pairing.as_mut() {
                p.dongle = match p.dongle {
                    DongleKind::HyperPolling => DongleKind::MouseDockPro,
                    DongleKind::MouseDockPro => DongleKind::Productivity,
                    DongleKind::Productivity => DongleKind::HyperPolling,
                };
            }
        });
    }

    // ==================== 音效 ====================

    /// 直接设定音效增强（`AUDIO_ENHANCEMENT`）。
    pub fn set_enhancement(
        &mut self,
        value: crate::domain::AudioEnhancement,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.sound.as_mut() {
                s.enhancement = value;
            }
        });
    }

    /// 直接设定主音量。
    pub fn set_volume(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.sound.as_mut().map(|s| &mut s.volume));
    }

    pub fn adjust_volume(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.sound.as_mut().map(|s| &mut s.volume));
    }

    /// 直接设定游戏 / 聊天混音。
    pub fn set_chat_mix(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.sound.as_mut().map(|s| &mut s.chat_mix));
    }

    pub fn adjust_chat_mix(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.sound.as_mut().map(|s| &mut s.chat_mix));
    }

    pub fn toggle_audio_meter(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.sound.as_mut().map(|s| &mut s.audio_meter));
    }

    pub fn toggle_audio_mirroring(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.sound.as_mut().map(|s| &mut s.audio_mirroring));
    }

    pub fn toggle_audio_power_saving(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.sound.as_mut().map(|s| &mut s.power_saving));
    }

    pub fn toggle_eq(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.sound.as_mut().map(|s| &mut s.equalizer.enabled)
        });
    }

    pub fn toggle_eq_esports(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.sound.as_mut().map(|s| &mut s.equalizer.esports)
        });
    }

    /// 调整某个均衡器频段的增益（dB）。
    pub fn adjust_eq_band(&mut self, index: usize, delta: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.sound.as_mut() {
                if let Some(band) = s.equalizer.bands.get_mut(index) {
                    *band = (*band as i32 + delta).clamp(-12, 12) as i8;
                }
            }
        });
    }

    pub fn set_eq_band(&mut self, index: usize, value: f32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(s) = f.sound.as_mut() {
                if let Some(band) = s.equalizer.bands.get_mut(index) {
                    *band = value.clamp(-12., 12.) as i8;
                }
            }
        });
    }

    // ==================== 麦克风 ====================

    /// 调整麦克风增益（0–100）。
    pub fn adjust_mic_gain(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.mic.as_mut().map(|m| &mut m.gain));
    }

    pub fn set_mic_gain(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.mic.as_mut().map(|m| &mut m.gain));
    }

    /// 循环切换麦克风增强档位（0–2）。
    pub fn cycle_mic_boost(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(m) = f.mic.as_mut() {
                m.boost = (m.boost + 1) % 3;
            }
        });
    }

    /// 调整侧音电平（0–100）。
    pub fn adjust_sidetone(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.mic.as_mut().map(|m| &mut m.sidetone));
    }

    pub fn set_sidetone(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.mic.as_mut().map(|m| &mut m.sidetone));
    }

    pub fn toggle_mic_monitoring(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.mic.as_mut().map(|m| &mut m.monitoring));
    }

    pub fn toggle_mic_mute(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.mic.as_mut().map(|m| &mut m.muted));
    }

    pub fn toggle_ai_noise_cancellation(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.mic.as_mut().map(|m| &mut m.ai_noise_cancellation)
        });
    }

    pub fn toggle_high_pass_filter(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.mic.as_mut().map(|m| &mut m.high_pass_filter));
    }

    pub fn toggle_analogue_gain_limiter(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.mic.as_mut().map(|m| &mut m.analogue_gain_limiter)
        });
    }

    /// 直接设定采样率 44.1 / 48 / 96 kHz。
    pub fn set_sampling_rate(
        &mut self,
        rate: crate::domain::SamplingRate,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(m) = f.mic.as_mut() {
                m.sampling_rate = rate;
            }
        });
    }

    // ==================== 显示 / OLED ====================

    /// 循环切换屏幕刷新率（`REFRESH_RATE`）。
    pub fn cycle_refresh_rate(&mut self, step: i32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(d) = f.display.as_mut() {
                let rates = d.refresh_rates.clone();
                let current = rates.iter().position(|r| *r == d.refresh_rate).unwrap_or(0);
                if !rates.is_empty() {
                    let next = (current as i32 + step).rem_euclid(rates.len() as i32) as usize;
                    d.refresh_rate = rates[next];
                }
            }
        });
    }

    /// 直接设定屏幕刷新率。
    pub fn set_refresh_rate(&mut self, rate: u32, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(d) = f.display.as_mut() {
                d.refresh_rate = rate;
            }
        });
    }

    pub fn toggle_display_performance_mode(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| {
            f.display.as_mut().map(|d| &mut d.performance_mode)
        });
    }

    /// 调整 OLED 屏幕亮度（0–100）。
    pub fn adjust_oled_brightness(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.oled.as_mut().map(|o| &mut o.brightness));
    }

    pub fn set_oled_brightness(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.oled.as_mut().map(|o| &mut o.brightness));
    }

    /// 调整 OLED 闲置关闭时间（秒；0 = 从不）。
    pub fn adjust_oled_timeout(&mut self, delta: i32, cx: &mut Context<Self>) {
        use crate::domain::OLED_TIMEOUT_STEPS;
        self.edit_features(cx, |f| {
            if let Some(o) = f.oled.as_mut() {
                let current =
                    OLED_TIMEOUT_STEPS.iter().position(|v| *v == o.timeout_sec).unwrap_or(0)
                        as i32;
                let next =
                    (current + delta).rem_euclid(OLED_TIMEOUT_STEPS.len() as i32) as usize;
                o.timeout_sec = OLED_TIMEOUT_STEPS[next];
            }
        });
    }

    pub fn toggle_oled_animation(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.oled.as_mut().map(|o| &mut o.custom_animation));
    }

    // ==================== 触觉 ====================

    pub fn toggle_audio_to_haptics(&mut self, cx: &mut Context<Self>) {
        self.flip(cx, |f| f.haptics.as_mut().map(|h| &mut h.audio_to_haptics));
    }

    /// 调整触觉增益等级（0–100）。
    pub fn adjust_haptics_gain(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.haptics.as_mut().map(|h| &mut h.gain));
    }

    pub fn set_haptics_gain(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.haptics.as_mut().map(|h| &mut h.gain));
    }

    /// 调整触觉强度（0–100）。
    pub fn adjust_haptics_intensity(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.bump_u8(cx, delta, 5, |f| f.haptics.as_mut().map(|h| &mut h.intensity));
    }

    pub fn set_haptics_intensity(&mut self, value: f32, cx: &mut Context<Self>) {
        self.set_percent_u8(cx, value, |f| f.haptics.as_mut().map(|h| &mut h.intensity));
    }

    // ==================== 宏 ====================

    /// 新增一个宏。
    pub fn add_macro(&mut self, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            let n = f.macros.len() + 1;
            f.macros.push(Macro {
                name: format!("宏 {n}"),
                steps: Vec::new(),
                loop_until_release: false,
            });
        });
    }

    /// 删除第 `index` 个宏。
    pub fn remove_macro(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if index < f.macros.len() {
                f.macros.remove(index);
            }
        });
    }

    /// 给第 `index` 个宏追加一步。
    pub fn add_macro_step(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit_features(cx, |f| {
            if let Some(mac) = f.macros.get_mut(index) {
                mac.steps.push(MacroStep {
                    delay_ms: 20,
                    action: MACRO_ACTIONS[0].to_string(),
                });
            }
        });
    }

    /// 循环切换第 `index` 个宏、第 `step` 步的事件（`MACRO_ACTIONS`）。
    pub fn cycle_macro_action(
        &mut self,
        index: usize,
        step: usize,
        delta: i32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(entry) = f
                .macros
                .get_mut(index)
                .and_then(|mac| mac.steps.get_mut(step))
            {
                entry.action = cycle_str(&MACRO_ACTIONS, &entry.action, delta);
            }
        });
    }

    /// 直接设定第 `index` 个宏、第 `step` 步的事件。
    ///
    /// `MACRO_ACTIONS` 是固定集合 → 真实界面用 `.s3-dropdown` 直选。
    pub fn set_macro_action(
        &mut self,
        index: usize,
        step: usize,
        picked: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = MACRO_ACTIONS.get(picked) else {
            return;
        };
        let action = action.to_string();
        self.edit_features(cx, |f| {
            if let Some(entry) = f
                .macros
                .get_mut(index)
                .and_then(|mac| mac.steps.get_mut(step))
            {
                entry.action = action;
            }
        });
    }

    /// 直接设定第 `index` 个宏、第 `step` 步的前置延迟（毫秒）。
    pub fn set_macro_delay(
        &mut self,
        index: usize,
        step: usize,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(entry) = f
                .macros
                .get_mut(index)
                .and_then(|mac| mac.steps.get_mut(step))
            {
                entry.delay_ms = value.clamp(0., 60_000.) as u32;
            }
        });
    }

    /// 调整第 `index` 个宏、第 `step` 步的前置延迟（毫秒）。
    pub fn adjust_macro_delay(
        &mut self,
        index: usize,
        step: usize,
        delta: i32,
        cx: &mut Context<Self>,
    ) {
        self.edit_features(cx, |f| {
            if let Some(entry) = f
                .macros
                .get_mut(index)
                .and_then(|mac| mac.steps.get_mut(step))
            {
                entry.delay_ms = (entry.delay_ms as i32 + delta * 10).clamp(0, 5000) as u32;
            }
        });
    }
}

/// 用 `--tab` 之类的字符串解析重排首页分段时用的辅助（保持与 `HomeSection::ALL` 同序）。
pub fn default_home_sections() -> Vec<(HomeSection, bool)> {
    HomeSection::ALL
        .into_iter()
        .map(|section| (section, section.default_expanded()))
        .collect()
}














