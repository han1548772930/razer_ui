//! 尚未实现的标签页占位。
//!
//! 占位页会列出该标签页在雷云里**真实存在的 i18n key**，
//! 明确标出「这里应该有什么」，而**不是我自己编一个界面顶上**。
//!
//! key 来源：`.ref/notes/synapse-i18n-keys.txt`（从雷云前端代码缓存提取），
//! 缺口清单见 `docs/FEATURES.md` §7。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::nav::Tab;
use crate::pages::widgets::{card_title, card, EmptyState, PageLayout};

/// 渲染占位页。
///
/// 走共享的 [`PageLayout`]，因此占位页与已实现页**骨架一致**：
/// 同样的页头、同样的 250px 产品图区、同样的 600px 卡片列。
pub fn render(tab: Tab, device_name: &str, _cx: &mut Context<AppShell>) -> AnyElement {
    let keys = tab.evidence_keys();

    PageLayout::new(tab.zh(), device_name)
        .subtitle(format!("真实 key：{} · 该标签页尚未实现", tab.key()))
        .widget(
            card()
                .child(card_title("这个标签页在雷云里应有的内容"))
                .child(div().text_xs().child(
                    "下列 key 提取自雷云前端的代码缓存，是它真实使用的文案键名，不是推测",
                ))
                .when(keys.is_empty(), |this| {
                    this.child(div().text_sm().child("（尚未提取到该标签页的具体 key）"))
                })
                .children(
                    keys.iter()
                        .map(|key| div().text_sm().child(format!("· {key}")).into_any_element()),
                ),
        )
        .widget(EmptyState::new(
            "实现这些功能需要先与雷云建立对应关系；缺口清单见 docs/FEATURES.md §7。",
        ))
        .into_any_element()
}