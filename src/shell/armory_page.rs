//! Armory（`/synapse/armory/`）窗口。
//!
//! # 依据
//!
//! 窗口名、创建参数、URL 模板与查询参数、`displayMode=armory` 根分支、导航数组、
//! 标题 key 与图标，全部由 [audit-armory-app.cjs](../../tools/audit-armory-app.cjs)
//! 从当前源码静态提取，见 [armory 应用审计](../../docs/re/armory-app-current-audit.json)。
//!
//! - 窗口名常量：共享常量模块里 `"macro"→"armory"` 相邻定义；
//! - 创建参数：`policy=5,tab_visible=0,app_name=synapse,width=1280,height=720,
//!   minimum_width=600,minimum_height=500,app_icon_path=Synapse\window.ico`；
//! - 根分支：`"armory" === displayMode` 时 `body-wrapper` 追加 ` custom-scrollable`；
//! - 导航数组（顺序即源码顺序）：`[{id:ftd},{id:Hzf},{id:Vb_},{id:C9E}]`
//!   → `SPOTLIGHT_HEADER`(精选推荐) / `BROWSE_HEADER`(浏览) /
//!     `MY_DOWNLOADS_HEADER`(我的下载) / `MY_UPLOADS_HEADER`(我的上传)；
//! - 标题：`isExchangeEnabled ? DASHBOARD_EXCHANGE : DASHBOARD_WORKSHOP`（中文均为「互换」）。
//!
//! # 未接入的部分
//!
//! Armory 的资料分享服务（`armoryProfiles`、Razer ID、上传/下载/浏览/精选内容、
//! 分享弹层）尚未连接，界面不显示任何资料数据；导航的显示条件按源码记录在
//! [`ArmoryTab`] 上（未登录隐藏「我的上传」，未启用 phase-1 还会隐藏「精选推荐」），
//! 本地无这两项状态，因此四项都画出来并在正文里标注。
use crate::{i18n, ui::surface};
use gpui_kit::component::*;
use gpui_kit::*;

/// 原版导航数组的四项；`text_key` 即源码里的文案 key。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArmoryTab {
    Spotlight,
    Browse,
    MyDownloads,
    MyUploads,
}
impl ArmoryTab {
    /// 源码数组顺序：`ftd`、`Hzf`、`Vb_`、`C9E`。
    pub(super) const ALL: [Self; 4] = [
        Self::Spotlight,
        Self::Browse,
        Self::MyDownloads,
        Self::MyUploads,
    ];
    pub(super) fn text_key(self) -> &'static str {
        match self {
            Self::Spotlight => "SPOTLIGHT_HEADER",
            Self::Browse => "BROWSE_HEADER",
            Self::MyDownloads => "MY_DOWNLOADS_HEADER",
            Self::MyUploads => "MY_UPLOADS_HEADER",
        }
    }
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Spotlight => "armory-tab-spotlight",
            Self::Browse => "armory-tab-browse",
            Self::MyDownloads => "armory-tab-my-downloads",
            Self::MyUploads => "armory-tab-my-uploads",
        }
    }
    /// 源码过滤条件：`(!re || id !== C9E) && !!(Te || (id !== C9E && id !== ftd))`。
    /// `re` 是初值为 `true` 的本地 state，`Te = isPhase1FeaturesEnabled`。
    fn hidden_by_source_flags(self) -> bool {
        matches!(self, Self::MyUploads | Self::Spotlight)
    }
}

pub(super) struct ArmoryPage {
    tab: ArmoryTab,
    focus: FocusHandle,
}
impl ArmoryPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tab: ArmoryTab::Spotlight,
            focus: cx.focus_handle(),
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn set_tab(&mut self, tab: ArmoryTab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            cx.notify();
        }
    }
}
impl Render for ArmoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab;
        v_flex()
            .id("armory-window")
            .size_full()
            // `"armory" === displayMode` 时原版给 `body-wrapper` 追加 `custom-scrollable`。
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .id("armory-nav-tabs")
                    .flex_shrink_0()
                    .h(surface::css(48.))
                    .items_center()
                    .gap(surface::css(20.))
                    .px(surface::css(20.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children(ArmoryTab::ALL.into_iter().map(|entry| {
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
                    .id("armory-body")
                    .flex_1()
                    .min_h_0()
                    .gap(surface::css(12.))
                    .p(surface::css(20.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child(match tab {
                                ArmoryTab::Spotlight => {
                                    "原版「精选推荐」由 Armory 服务下发精选资料列表；本地未连接该服务，因此不显示任何资料。"
                                }
                                ArmoryTab::Browse => {
                                    "原版「浏览」按设备与分类浏览 Armory 资料（含搜索框，输入 300ms 后才发起查询）；本地未连接该服务。"
                                }
                                ArmoryTab::MyDownloads => {
                                    "原版「我的下载」列出当前账户已下载的资料；本地未连接账户与 Armory 服务。"
                                }
                                ArmoryTab::MyUploads => {
                                    "原版「我的上传」需要 Razer ID，并且是源码里默认隐藏的一项（本地 state 初值 true）；本地未连接账户服务。"
                                }
                            }),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} · {} · 源码条件：未登录隐藏 / 未启用 phase-1 隐藏",
                                tab.text_key(),
                                if tab.hidden_by_source_flags() {
                                    "默认隐藏"
                                } else {
                                    "默认显示"
                                }
                            )),
                    ),
            )
            .track_focus(&self.focus)
    }
}
