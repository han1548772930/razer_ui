//! 宏应用窗口（`/synapse/macro/`）。
//!
//! 打开点、窗口名与打开标志：见 [窗口打开契约](../../docs/re/display-window-contract.md)
//! 的 `named_windows`（`{windowName:s.BDG, url:"/synapse/macro/", openParam:"policy=3,tab_visible=1"}`）
//! 与[宏应用审计](../../docs/re/macro-app-current-audit.md)。
//!
//! 外框类名、两个导航标签的 key 与文案，以及功能面板（palette）的条目与缺口，由
//! [extract-macro-app-ui.cjs](../../tools/extract-macro-app-ui.cjs) 静态提取，见
//! [宏应用界面审计](../../docs/re/macro-app-ui-audit.md)。
//!
//! 宏服务尚未连接：界面不显示任何宏数据，也不把未知的模块安装状态写成已安装。
use crate::{i18n, ui::surface};
use gpui_kit::component::*;
use gpui_kit::*;

/// 原版的两个导航标签（`TEXT_NAV_TAB_MY_MACROS`、`TEXT_NAV_TAB_KEY_BINDS`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacroTab {
    MyMacros,
    KeyBinds,
}
impl MacroTab {
    const ALL: [Self; 2] = [Self::MyMacros, Self::KeyBinds];
    fn text_key(self) -> &'static str {
        match self {
            Self::MyMacros => "TEXT_NAV_TAB_MY_MACROS",
            Self::KeyBinds => "TEXT_NAV_TAB_KEY_BINDS",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::MyMacros => "macro-tab-my-macros",
            Self::KeyBinds => "macro-tab-key-binds",
        }
    }
}

/// 原版功能面板（palette）的条目：类型、图标文件与文案 key 都来自宏应用的
/// 模块 81021。图标资源在当前源码包中不存在（宏应用只取到了 JS/CSS），因此
/// 面板尚未绘制；这里只记录已核对的清单，不用别的图标代替。
const PALETTE: &[(&str, &str, &str)] = &[
    (
        "delay",
        "static/media/icon_delay_g.5050a3f7.svg",
        "TEXT_ADD_MENU_DELAY",
    ),
    (
        "keyboard",
        "static/media/icon_config_keyboard_a.7051c99b.svg",
        "TEXT_ADD_MENU_KEYBOARD",
    ),
    (
        "mouse",
        "static/media/icon_config_mouse_o.8a44fbe7.svg",
        "TEXT_ADD_MENU_MOUSE_FUNCTION",
    ),
    (
        "macro",
        "static/media/icon_macro_a.7e1bc94f.svg",
        "TEXT_ADD_MENU_MACRO",
    ),
    (
        "launch",
        "static/media/icon_config_launch_p.482fbff5.svg",
        "TEXT_ADD_MENU_LAUNCH",
    ),
    (
        "command",
        "static/media/icon_runcmd_b.10c00024.svg",
        "TEXT_ADD_MENU_RUN_COMMAND",
    ),
    (
        "text",
        "static/media/icon_config_text_b.bc93ac89.svg",
        "TEXT_ADD_MENU_TEXT_FUNCTION",
    ),
    (
        "loop",
        "static/media/icon_refresh-1_r.ff48f955.svg",
        "TEXT_ADD_MENU_LOOP",
    ),
    (
        "ai_rephrase",
        "static/media/ai_rephrase.ce435691.svg",
        "TEXT_AI_REPHRASE",
    ),
    (
        "ai_summarize",
        "static/media/ai_summarize.953171a6.svg",
        "TEXT_AI_SUMMARIZE",
    ),
    (
        "ai_email_composer",
        "static/media/ai_compose_email.9888530a.svg",
        "TEXT_AI_COMPOSE_EMAIL",
    ),
];

pub(super) struct MacroPage {
    tab: MacroTab,
    focus: FocusHandle,
}
impl MacroPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tab: MacroTab::MyMacros,
            focus: cx.focus_handle(),
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn set_tab(&mut self, tab: MacroTab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            cx.notify();
        }
    }
}
impl Render for MacroPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 原版外层是 `.MacroContainer_my_macro__jCmj8{position:static}`：容器本身
        // 不加定位，页面继承应用背景。`setup_svgs` 子元素是 `display:none` 的
        // 图标预载容器，不绘制。
        let tab = self.tab;
        v_flex()
            .id("macro-window")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                // 原版宏窗口顶部是 `.nav-wrapper{background:#222;width:100vw}` +
                // `.nav-wrapper.over-border{border-bottom:2px solid #000}`，里面
                // `.module-nav{align-items:center;display:flex;flex:1 0 max-content;
                //  height:46px;justify-content:center}`（模块导航居中）。
                h_flex()
                    .id("macro-nav-tabs")
                    .flex_shrink_0()
                    .h(surface::css(46.))
                    .items_center()
                    .justify_center()
                    .gap(surface::css(20.))
                    .bg(cx.theme().sidebar)
                    .border_b_2()
                    .border_color(cx.theme().title_bar)
                    .children(MacroTab::ALL.into_iter().map(|entry| {
                        surface::navigation_button(
                            SharedString::from(entry.id()),
                            i18n::t(entry.text_key()),
                            entry == tab,
                            cx,
                        )
                        .role(Role::Tab)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_tab(entry, cx)))
                    })),
            )
            .child(
                v_flex()
                    .id("macro-body")
                    .flex_1()
                    .min_h_0()
                    .gap(surface::css(16.))
                    .p(surface::css(20.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child(match tab {
                                MacroTab::MyMacros => {
                                    "宏服务尚未连接：本地没有宏列表数据，因此这里不显示任何宏，也不把未知的模块安装状态写成已安装。原版此页左侧是功能面板（delay、keyboard、mouse、macro、launch、command、text、loop 与 3 个 AI 项），其图标资源在当前源码包中不存在，尚未绘制。"
                                }
                                MacroTab::KeyBinds => {
                                    "原版「按键绑定」标签页渲染 KeyBindContainer 按键绑定容器，复用设备的按键映射界面；本地的按键映射在设备窗口的「自定义」页提供。"
                                }
                            }),
                    )
                    .child(
                        v_flex()
                            .id("macro-palette-gaps")
                            .gap(surface::css(4.))
                            .children(PALETTE.iter().map(|(kind, media, key)| {
                                div()
                                    .text_size(surface::css(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{kind} · {key} · {media}"))
                            })),
                    ),
            )
            .track_focus(&self.focus)
    }
}
