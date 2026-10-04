//! Profiles（`/synapse/profiles/`）窗口。
//!
//! # 依据
//!
//! 模块表（`.ref/applications/rz-app-menu/static/js/main.83ced465.js`，见
//! [module-registry-audit.json](../../docs/re/module-registry-audit.json)）里：
//!
//! ```js
//! {moduleName:"linkedGames", windowName:"profiles", url:"/synapse/profiles/",
//!  openParam:[ZP.sameWindow, ZP.autoFocus, ZP.tabVisible]}
//! ```
//!
//! 其中 `ZP.sameWindow="policy=3"`、`ZP.autoFocus="shouldFocus=1"`、
//! `ZP.tabVisible="tab_visible=1"`：即**同一窗口 + 聚焦 + 在标签栏出现**。
//! 因此「已关联的游戏」入口打开的是这个 `profiles` 窗口，而不是新进程窗口。
//!
//! # 未接入的部分
//!
//! profiles 应用本体（`.ref/applications/synapse/profiles/`）的各视图内容尚未实现：
//! 配置文件列表与编辑、本地/云配置文件、导入导出、通用快捷键、以及已关联游戏列表。
//! 应用自己的**路由**可以从源码常量里取到（`H=/\/devices\/\d+\//`、
//! `b=/\/globalShortcuts\//`、`F=/\/chromaStudio\//`、`B=/\/macro\//`、
//! `k=/\/linkedGames\//`），因此左侧按这些路由列出视图；每个视图的正文目前是依据说明。
use crate::{
    i18n,
    ui::{game_tile, surface},
};
use gpui_kit::component::*;
use gpui_kit::*;

pub(super) struct ProfilesPage {
    focus: FocusHandle,
    status: String,
    view: ProfilesView,
    /// profiles 应用的导航栏（源码 `Xt` 组件）自己维护历史：chunk 里是
    /// `tabNavigation` / `currentTabNavigation` + `navigateBack` / `navigateForward`，
    /// 标签栏最左边就是 `.nav.back` / `.nav.forward`。
    history: Vec<ProfilesView>,
    history_index: usize,
    history_navigation: bool,
}

/// profiles 应用自己的路由（源码常量正则，见模块文档）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfilesView {
    Devices,
    GlobalShortcuts,
    ChromaStudio,
    Macro,
    LinkedGames,
}

impl ProfilesView {
    const ALL: [Self; 5] = [
        Self::Devices,
        Self::GlobalShortcuts,
        Self::ChromaStudio,
        Self::Macro,
        Self::LinkedGames,
    ];
    /// 源码里对应的路由正则原文。
    fn route(self) -> &'static str {
        match self {
            Self::Devices => r"H=/\/devices\/\d+\//",
            Self::GlobalShortcuts => r"b=/\/globalShortcuts\//",
            Self::ChromaStudio => r"F=/\/chromaStudio\//",
            Self::Macro => r"B=/\/macro\//",
            Self::LinkedGames => r"k=/\/linkedGames\//",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Devices => "profiles-view-devices",
            Self::GlobalShortcuts => "profiles-view-global-shortcuts",
            Self::ChromaStudio => "profiles-view-chroma-studio",
            Self::Macro => "profiles-view-macro",
            Self::LinkedGames => "profiles-view-linked-games",
        }
    }
    fn label(self) -> String {
        match self {
            Self::Devices => i18n::t_or("PROFILES", "配置文件"),
            Self::GlobalShortcuts => i18n::t_or("GLOBAL_SHORTCUT_HEADER", "通用快捷键"),
            Self::ChromaStudio => i18n::t_or("CHROMA_STUDIO", "幻彩控制室"),
            Self::Macro => i18n::t_or("MACRO", "宏"),
            Self::LinkedGames => i18n::t_or("LINKED_GAMES", "已关联的游戏"),
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Devices => {
                "原版按 `/devices/<设备号>/` 展示该设备的配置文件设置（本地/云配置文件、导入导出）。本地未接入配置文件数据，故不显示任何配置文件。"
            }
            Self::GlobalShortcuts => {
                "原版 `/globalShortcuts/` 展示通用快捷键列表；本地未接入通用快捷键数据。"
            }
            Self::ChromaStudio => "原版 `/chromaStudio/` 打开幻彩控制室；本地未接入该应用。",
            Self::Macro => "原版 `/macro/` 打开宏应用；本地宏窗口见标签栏里的「宏」。",
            Self::LinkedGames => {
                "原版 `/linkedGames/` 就是本窗口的已关联游戏视图：`.list-box` 里一张张 `.linked-game-tile`，末尾固定跟一张 `.add-new` 虚线磁贴。本地未接入已关联游戏数据。"
            }
        }
    }
}
impl ProfilesPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            status: String::new(),
            view: ProfilesView::LinkedGames,
            history: vec![ProfilesView::LinkedGames],
            history_index: 0,
            history_navigation: false,
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    /// `/linkedGames/` 视图的正文：源码里 `.list-box` + 末尾固定一张 `.add-new`
    /// 虚线磁贴；本地没有已关联游戏数据，所以只有这张磁贴。
    fn view_body(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut children = Vec::new();
        if self.view == ProfilesView::LinkedGames {
            children.push(game_tile::list_box([game_tile::add_new_tile(
                "linked-games-add-new",
                game_tile::add_new_label(),
                cx.listener(|this, _, _, cx| {
                    this.status = "游戏扫描与选择弹层尚未接入。".into();
                    cx.notify();
                }),
                cx,
            )]));
        }
        if !self.status.is_empty() {
            children.push(surface::note(self.status.clone(), cx).into_any_element());
        }
        children
    }
    fn set_view(&mut self, view: ProfilesView, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        self.view = view;
        if std::mem::take(&mut self.history_navigation) {
            // 历史回放：索引已在 step_history 里改好。
        } else {
            self.history.truncate(self.history_index + 1);
            self.history.push(view);
            self.history_index = self.history.len() - 1;
        }
        cx.notify();
    }
    fn can_step_history(&self, forward: bool) -> bool {
        if forward {
            self.history_index + 1 < self.history.len()
        } else {
            self.history_index > 0
        }
    }
    fn step_history(&mut self, forward: bool, cx: &mut Context<Self>) {
        if !self.can_step_history(forward) {
            return;
        }
        let index = if forward {
            self.history_index + 1
        } else {
            self.history_index - 1
        };
        let view = self.history[index];
        self.history_index = index;
        self.history_navigation = true;
        self.set_view(view, cx);
    }
}
impl Render for ProfilesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("profiles-window")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .child(
                h_flex()
                    .id("profiles-header")
                    .flex_shrink_0()
                    .h(surface::css(48.))
                    .items_center()
                    .px(surface::css(20.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    // 源码 `Xt` 导航栏的最左边是 `.nav.back` / `.nav.forward`
                    // （`background-image` 分别是 nav_back_arrow / nav_fwd_arrow，
                    // 到边界时 `.nav.disabled{opacity:.3}`）。
                    .child(
                        surface::nav_arrow_button(
                            "profiles-back",
                            false,
                            self.can_step_history(false),
                            cx,
                        )
                        .ml(surface::css(20.))
                        .on_click(cx.listener(|this, _, _, cx| this.step_history(false, cx))),
                    )
                    .child(
                        surface::nav_arrow_button(
                            "profiles-forward",
                            true,
                            self.can_step_history(true),
                            cx,
                        )
                        .ml(surface::css(20.))
                        .on_click(cx.listener(|this, _, _, cx| this.step_history(true, cx))),
                    )
                    // `.navs-wrapper{display:flex;font-size:12px;justify-content:center}`：
                    // 视图标签之间 20px（`.nav-tabs .nav{margin-right:20px}`）。
                    .child(
                        h_flex()
                            .id("profiles-navs")
                            .flex()
                            .items_center()
                            .gap(surface::css(20.))
                            .ml(surface::css(20.))
                            .children(ProfilesView::ALL.into_iter().map(|view| {
                                surface::navigation_button(
                                    SharedString::from(view.id()),
                                    view.label(),
                                    view == self.view,
                                    cx,
                                )
                                .role(Role::Tab)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.set_view(view, cx)),
                                )
                            })),
                    ),
            )
            .child(
                // 源码把视图容器标成 `.razer-profiles`
                // （`.razer-profiles{overflow:hidden!important;padding:0!important}`），
                // 即这一层不滚动也不留内边距，间距由各视图自己给。
                v_flex()
                    .id("profiles-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .p_0()
                    // `.razer-profiles` 这一层不留内边距；真实视图各自管自己的
                    // 间距，本地这些说明文字统一放进一个有内边距的内层容器。
                    .child(
                        v_flex()
                            .id("profiles-view")
                            .size_full()
                            .gap(surface::css(12.))
                            .p(surface::css(20.))
                            .child(
                        div()
                            .text_size(surface::css(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.view.route()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.view.description()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("窗口名 profiles · /synapse/profiles/ · policy=3,shouldFocus=1,tab_visible=1"),
                    )
                            .children(self.view_body(cx)),
                    ),
            )
    }
}
